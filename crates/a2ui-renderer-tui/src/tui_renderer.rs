use crate::focus_manager::FocusManager;
use crate::{
    widget_builder::{component_style_to_tui, RenderableWidget},
    WidgetBuilder, WidgetMapper,
};
use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_renderer::{
    CatalogRegistry, ComponentForest, CustomComponentRegistry, DataBinding, DependencyGraph,
    FunctionDispatcher, PathResolver, RenderResult, Renderer, SurfaceHandle, SurfaceLru, UserEvent,
};
use ratatui::{
    layout::Rect,
    widgets::{Block, Paragraph},
    Frame,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// TUI 渲染器实现
#[derive(Debug)]
pub struct TuiRenderer {
    /// Surface 句柄 → SurfaceId 映射
    pub surfaces: HashMap<SurfaceHandle, String>,
    /// 组件森林（所有 Surface 的组件树）
    forest: ComponentForest,
    /// DataModel 绑定（使用字符串作为 Surface 标识）
    data_bindings: HashMap<String, DataBinding>,
    /// 依赖图
    dependency_graph: DependencyGraph,
    /// 函数调度器（用于 callableFrom enforcement）
    dispatcher: FunctionDispatcher,
    /// Catalog 注册表（用于 catalogId 信任链校验）
    catalog_registry: CatalogRegistry,
    /// 焦点管理器
    pub focus_manager: FocusManager,
    /// P1-2: action_id → response_path 映射（responsePath 写回用）
    pending_responses: HashMap<String, String>,
    /// P2-2: Surface 的 sendDataModel 标记（为 true 时 action 附带完整 data model）
    send_data_model: HashMap<String, bool>,
    /// P4-1: 需要增量重渲染的 surface 集合
    dirty_surfaces: HashSet<String>,
    /// Surface LRU 驱逐管理器
    surface_lru: SurfaceLru,
    /// 自定义组件注册表
    custom_registry: CustomComponentRegistry,
    /// 最近一帧构建的 widget 数量（render() 填充，测试用）
    pub last_frame_widget_count: usize,
}

/// 最大并发 Surface 数量（DoS 防护）
const MAX_SURFACES: usize = 100;
/// 单 Surface 最大组件数量（DoS 防护）
const MAX_COMPONENTS_PER_SURFACE: usize = 1000;

impl TuiRenderer {
    /// 创建新的 TUI 渲染器
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            forest: ComponentForest::new(),
            data_bindings: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            dispatcher: FunctionDispatcher::new(),
            catalog_registry: CatalogRegistry::with_defaults(),
            focus_manager: FocusManager::new(),
            pending_responses: HashMap::new(),
            send_data_model: HashMap::new(),
            dirty_surfaces: HashSet::new(),
            surface_lru: SurfaceLru::new(MAX_SURFACES, Some(Duration::from_secs(600))),
            custom_registry: CustomComponentRegistry::new(),
            last_frame_widget_count: 0,
        }
    }

    /// 获取依赖图的只读引用（用于测试和查询）
    pub fn dependency_graph(&self) -> &DependencyGraph {
        &self.dependency_graph
    }

    /// 注册客户端函数（供 callableFrom enforcement 使用）
    pub fn register_function(
        &mut self,
        name: impl Into<String>,
        callable_from: a2ui_renderer::CallableFrom,
    ) {
        self.dispatcher.register(name, callable_from);
    }

    /// 获取已注册函数列表
    pub fn registered_functions(&self) -> Vec<&String> {
        self.dispatcher.registered_names()
    }

    /// 注册 Catalog（用于 catalogId 信任链校验）
    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.catalog_registry.register(catalog)
    }

    /// 获取 Catalog 注册表的只读引用
    pub fn catalog_registry(&self) -> &CatalogRegistry {
        &self.catalog_registry
    }

    /// 注册自定义组件类型
    pub fn register_custom_component(
        &mut self,
        def: a2ui_renderer::CustomComponentDef,
    ) -> Result<(), String> {
        self.custom_registry.register(def)
    }

    // P1-2: 注册待响应的 action_id → response_path 映射
    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.pending_responses
            .insert(action_id.into(), response_path.into());
    }

    /// 使用 ratatui Terminal 执行实际帧绘制
    pub async fn render_frame<B>(&mut self, terminal: &mut ratatui::Terminal<B>) -> RenderResult<()>
    where
        B: ratatui::backend::Backend,
    {
        // 帧准备：构建所有 surface 的 widget 树
        let surface_widgets = self.prepare_frame().await?;

        let widgets_to_draw: Vec<_> = surface_widgets
            .into_iter()
            .flat_map(|(_, widgets)| widgets)
            .collect();

        terminal
            .draw(|frame: &mut Frame| {
                for widget in &widgets_to_draw {
                    self.draw_widget(frame, widget.clone());
                }
            })
            .map_err(|e| {
                a2ui_renderer::error::RendererError::BindingError(format!(
                    "terminal draw error: {}",
                    e
                ))
            })?;

        self.dirty_surfaces.clear();
        Ok(())
    }

    /// 准备帧：构建所有 surface 的 widget 树
    /// 返回 (surface_id, widgets) 列表
    async fn prepare_frame(&mut self) -> RenderResult<Vec<(String, Vec<RenderableWidget>)>> {
        let surfaces_to_render: Vec<_> = if self.dirty_surfaces.is_empty() {
            self.surfaces.values().cloned().collect()
        } else {
            self.dirty_surfaces.iter().cloned().collect()
        };

        let mapper = WidgetMapper;
        let mut all_widgets = Vec::new();

        for surface_id in &surfaces_to_render {
            let binding = match self.data_bindings.get(surface_id) {
                Some(b) => b,
                None => continue,
            };

            let builder = WidgetBuilder::new(binding, &self.forest, &self.custom_registry);
            let area = Rect::new(0, 0, 80, 24);
            let widgets = builder.build_tree(surface_id, area);
            all_widgets.push((surface_id.clone(), widgets));
        }

        let total: usize = all_widgets.iter().map(|(_, w)| w.len()).sum();
        self.last_frame_widget_count = total;

        // 收集所有可聚焦组件 ID
        let mut focusable_ids = Vec::new();
        for (surface_id, _) in &all_widgets {
            for comp in self.forest.components_of(surface_id) {
                if mapper.is_focusable(comp) {
                    focusable_ids.push(comp.id().clone());
                }
            }
        }
        self.focus_manager.set_focusable(focusable_ids);

        Ok(all_widgets)
    }

    /// 将单个 RenderableWidget 绘制到 Frame
    fn draw_widget(&self, frame: &mut Frame, widget: RenderableWidget) {
        // 检查此 widget 是否为当前焦点组件
        let is_focused = self
            .focus_manager
            .current()
            .map(|focused_id| focused_id == widget.id())
            .unwrap_or(false);

        match widget {
            RenderableWidget::Paragraph {
                area, text, style, ..
            } => {
                let mut style = component_style_to_tui(&style);
                if is_focused {
                    style = style.add_modifier(ratatui::style::Modifier::REVERSED);
                }
                let para = Paragraph::new(text).style(style);
                frame.render_widget(para, area);
            }
            RenderableWidget::Block { area, title, .. } => {
                let block = Block::default().title(title);
                frame.render_widget(block, area);
            }
            RenderableWidget::Placeholder { area, reason, .. } => {
                let text = Paragraph::new(format!("[{}]", reason));
                frame.render_widget(text, area);
            }
            RenderableWidget::TextField {
                area,
                value,
                placeholder,
                ..
            } => {
                let display = if value.is_empty() {
                    placeholder.as_str()
                } else {
                    value.as_str()
                };
                let text = Paragraph::new(format!("[{}]", display));
                frame.render_widget(text, area);
            }
            RenderableWidget::CheckBox {
                area,
                label,
                checked,
                ..
            } => {
                let status = if checked { "[x]" } else { "[ ]" };
                let text = Paragraph::new(format!("{} {}", status, label));
                frame.render_widget(text, area);
            }
            RenderableWidget::Slider {
                area,
                value,
                min,
                max,
                ..
            } => {
                let range = max - min;
                let ratio = if range == 0.0 {
                    0.0
                } else {
                    ((value - min) / range).clamp(0.0, 1.0)
                };
                let filled = (ratio * 20.0).round() as usize;
                let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(20 - filled));
                let text = Paragraph::new(bar);
                frame.render_widget(text, area);
            }
            RenderableWidget::Button {
                area,
                label,
                variant,
                ..
            } => {
                let display = if variant == "primary" {
                    format!("[ {} ]", label)
                } else {
                    format!("< {} >", label)
                };
                let p = Paragraph::new(display);
                frame.render_widget(p, area);
            }
            RenderableWidget::Card { area, .. } => {
                let block = Block::default().title("┌─┐");
                frame.render_widget(block, area);
            }
            RenderableWidget::Divider { area, .. } => {
                let line = "─".repeat(area.width as usize);
                frame.render_widget(Paragraph::new(line), area);
            }
            RenderableWidget::Icon {
                area,
                symbol,
                style,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(symbol).style(component_style_to_tui(&style)),
                    area,
                );
            }
            RenderableWidget::Image { area, url, .. } => {
                frame.render_widget(Paragraph::new(format!("🖼 {}", url)), area);
            }
            RenderableWidget::Tabs { area, titles, .. } => {
                let header = titles.join(" │ ");
                frame.render_widget(Paragraph::new(header), area);
            }
            RenderableWidget::ChoicePicker {
                area,
                options,
                selected,
                ..
            } => {
                let display: Vec<String> = options
                    .iter()
                    .map(|o| {
                        if selected.contains(o) {
                            format!("(●) {}", o)
                        } else {
                            format!("( ) {}", o)
                        }
                    })
                    .collect();
                frame.render_widget(Paragraph::new(display.join("  ")), area);
            }
            RenderableWidget::Video { area, url, .. } => {
                frame.render_widget(Paragraph::new(format!("🎬 {}", url)), area);
            }
            RenderableWidget::AudioPlayer {
                area,
                url,
                description,
                ..
            } => {
                let text = if description.is_empty() {
                    format!("🔊 {}", url)
                } else {
                    format!("🔊 {} — {}", url, description)
                };
                frame.render_widget(Paragraph::new(text), area);
            }
            RenderableWidget::Modal {
                area,
                trigger_id,
                content_id,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "[Modal: trigger={} content={}]",
                        trigger_id, content_id
                    )),
                    area,
                );
            }
            RenderableWidget::DateTimeInput { area, label, .. } => {
                frame.render_widget(Paragraph::new(format!("📅 {}", label)), area);
            }
        }
    }

    /// 渲染单个组件到 Frame（简化实现）
    pub fn render_component(&self, component: &Component, frame: &mut Frame, area: Rect) {
        let mapper = WidgetMapper;
        let paragraph = mapper.map_to_paragraph(component);
        frame.render_widget(paragraph, area);
    }
}

/// 从组件的 properties 中递归提取所有 JSON Pointer 路径
fn extract_paths(component: &Component) -> Vec<String> {
    let mut paths = Vec::new();
    extract_paths_from_value(component.properties(), &mut paths);
    paths
}

/// 递归遍历 serde_json::Value，收集所有 DynamicValue::Path 中的路径字符串
fn extract_paths_from_value(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (_, v) in map {
                // 检测 {"path": "..."} 结构
                if let Value::Object(inner) = v {
                    if inner.len() == 1 {
                        if let Some(Value::String(p)) = inner.get("path") {
                            paths.push(p.clone());
                            continue;
                        }
                    }
                }
                extract_paths_from_value(v, paths);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_paths_from_value(item, paths);
            }
        }
        _ => {}
    }
}

impl Default for TuiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for TuiRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        tracing::trace!(surface_id = %msg.surface_id, "createSurface");
        // LRU 驱逐：检查是否需要驱逐最久未用的 surface
        if let Some(victim_id) = self.surface_lru.find_victim(self.surfaces.len()) {
            // 驱逐最久未用的 surface
            self.forest.remove_surface(&victim_id).ok();
            self.data_bindings.remove(&victim_id);
            self.surfaces.retain(|_, sid| sid != &victim_id);
            self.dirty_surfaces.remove(&victim_id);
            self.send_data_model.remove(&victim_id);
            self.surface_lru.remove(&victim_id);
        }

        // P0-3: enforcing surface limit（最后保护）
        if self.surfaces.len() >= MAX_SURFACES {
            return Err(a2ui_renderer::error::RendererError::SurfaceLimitExceeded {
                current: self.surfaces.len(),
                max: MAX_SURFACES,
            });
        }

        // P0-5: Catalog 信任链 — catalogId 校验
        // 如果已注册 Catalog，校验 catalogId 匹配；空注册表时跳过（向后兼容）
        if !self.catalog_registry.registered_ids().is_empty() {
            if !self.catalog_registry.has_catalog(&msg.catalog_id) {
                return Err(a2ui_renderer::error::RendererError::CatalogNotFound(
                    msg.catalog_id.clone(),
                ));
            }
        }

        let handle = SurfaceHandle::new();
        let surface_id = msg.surface_id.clone();

        // 注册组件
        if let Some(components) = msg.components.clone() {
            // P0-3: enforcing component limit
            let new_count = components.len();
            let existing_count = self.forest.component_count(&surface_id);
            if existing_count + new_count > MAX_COMPONENTS_PER_SURFACE {
                return Err(
                    a2ui_renderer::error::RendererError::ComponentLimitExceeded {
                        surface_id: surface_id.clone(),
                        current: existing_count + new_count,
                        max: MAX_COMPONENTS_PER_SURFACE,
                    },
                );
            }

            for comp in &components {
                self.forest.upsert(&surface_id, comp.clone())?;
            }
            // 注册依赖关系到 DependencyGraph
            for comp in components {
                for path in extract_paths(&comp) {
                    self.dependency_graph
                        .register_dependency(comp.id().clone(), path);
                }
            }
        }

        // 注册 DataModel
        let data_model = msg.data_model.unwrap_or(Value::Object(Default::default()));
        self.data_bindings.insert(
            surface_id.clone(),
            DataBinding::new(DataModel::new(data_model.clone())),
        );

        // P2-2: 记录 sendDataModel 标记
        self.send_data_model
            .insert(surface_id.clone(), msg.send_data_model);

        // P1-1: 展开 ChildList::Object 模板（@index 作用域系统）
        if let Some(binding) = self.data_bindings.get(&surface_id) {
            let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));
            self.forest
                .expand_templates(&surface_id, binding, &resolver, &self.dispatcher)?;
        }

        // 记录 Surface 映射
        self.surfaces.insert(handle, surface_id.clone());

        // 记录 LRU 访问
        self.surface_lru.touch(&surface_id);

        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        for comp in msg.components {
            self.forest.upsert(&surface_id, comp)?;
        }
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        tracing::debug!(surface_id = %msg.surface_id, path = ?msg.path, "updateDataModel");
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        if let Some(binding) = self.data_bindings.get_mut(&surface_id) {
            if let Some(path) = &msg.path {
                binding.set(path, msg.value.unwrap_or(Value::Null))?;
                // 查询依赖图，获取需要重渲染的组件
                let affected = self.dependency_graph.on_data_change(path);
                // 记录受影响的 surface 需要重渲染
                if !affected.is_empty() {
                    self.dirty_surfaces.insert(surface_id);
                }
            }
        }
        Ok(())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        tracing::info!(surface_id = %msg.surface_id, "deleteSurface");
        let surface_id = msg.surface_id.clone();
        self.forest.remove_surface(&surface_id)?;
        self.data_bindings.remove(&surface_id);
        self.dirty_surfaces.remove(&surface_id);
        self.send_data_model.remove(&surface_id);
        // 移除 surface 映射
        self.surfaces.retain(|_, sid| sid != &surface_id);
        // 移除 LRU 追踪
        self.surface_lru.remove(&surface_id);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        // P1-2: responsePath 写回 — 根据 action_id 查找 response_path 并写入 DataModel
        let action_id = msg.action_id.clone();
        if let Some(response_path) = self.pending_responses.remove(&action_id) {
            // 确定要写入的值
            let write_value = match &msg.response {
                a2ui_core::message::server_to_client::ActionResponsePayload::Success(v) => {
                    v.clone()
                }
                a2ui_core::message::server_to_client::ActionResponsePayload::Error(err) => {
                    Value::String(err.message.clone())
                }
            };

            // 写入 DataModel
            // 需要找到对应的 surface — 通过查找哪个 binding 包含该路径
            for (surface_id, binding) in self.data_bindings.iter_mut() {
                if binding.as_value().pointer(&response_path).is_some() || response_path == "/" {
                    self.surface_lru.touch(surface_id);
                    binding.set(&response_path, write_value)?;
                    // 查询依赖图，标记受影响的 surface 为脏
                    let affected = self.dependency_graph.on_data_change(&response_path);
                    if !affected.is_empty() {
                        self.dirty_surfaces.insert(surface_id.clone());
                    }
                    break;
                }
            }

            // 清理 pending response
            self.pending_responses.remove(&action_id);
        }
        Ok(())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        let function_name = msg.call.call.clone();
        // dispatch() 内部强制执行 callableFrom 边界检查
        let result = self.dispatcher.dispatch(
            &function_name,
            msg.call.args,
            a2ui_renderer::CallableFrom::ClientOnly,
        )?;
        Ok(FunctionResponse {
            function_call_id: msg.function_call_id,
            call: function_name,
            value: result,
        })
    }

    async fn render(&mut self) -> RenderResult<()> {
        // 帧准备：构建所有 surface 的 widget 树，验证组件引用和路径解析
        self.prepare_frame().await?;
        Ok(())
    }

    async fn handle_user_event(&mut self, event: UserEvent) -> RenderResult<Option<ActionMessage>> {
        // 确定性地查找 event 所属的 surface: 使用 component_id 反向索引
        let surface_for = |comp_id: &ComponentId| -> Option<String> {
            self.forest
                .surface_of(comp_id)
                .filter(|sid| self.send_data_model.get(*sid).copied().unwrap_or(false))
                .map(String::from)
        };

        match event {
            UserEvent::Click { component_id } => {
                let mut action = ActionMessage::event("click", "").with_context(
                    "source",
                    DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                );
                if let Some(surface_id) = surface_for(&component_id) {
                    if let Some(binding) = self.data_bindings.get(&surface_id) {
                        action = action.with_context(
                            "dataModel",
                            DynamicValue::Literal(binding.as_value().clone()),
                        );
                    }
                }
                Ok(Some(action))
            }
            UserEvent::KeyPress { key } => {
                match key.as_str() {
                    "Tab" | "Down" => {
                        self.focus_manager.next();
                        return Ok(None);
                    }
                    "Up" => {
                        self.focus_manager.previous();
                        return Ok(None);
                    }
                    "Enter" | " " => {
                        if let Some(comp_id) = self.focus_manager.current().cloned() {
                            let mut action = ActionMessage::event("activate", "").with_context(
                                "source",
                                DynamicValue::Literal(Value::String(comp_id.as_str().to_string())),
                            );
                            if let Some(surface_id) = surface_for(&comp_id) {
                                if let Some(binding) = self.data_bindings.get(&surface_id) {
                                    action = action.with_context(
                                        "dataModel",
                                        DynamicValue::Literal(binding.as_value().clone()),
                                    );
                                }
                            }
                            return Ok(Some(action));
                        }
                    }
                    _ => {}
                }
                Ok(None)
            }
            UserEvent::TextInput {
                component_id,
                value,
            } => {
                let mut action = ActionMessage::event("input", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context("value", DynamicValue::Literal(Value::String(value)));
                if let Some(surface_id) = surface_for(&component_id) {
                    if let Some(binding) = self.data_bindings.get(&surface_id) {
                        action = action.with_context(
                            "dataModel",
                            DynamicValue::Literal(binding.as_value().clone()),
                        );
                    }
                }
                Ok(Some(action))
            }
            UserEvent::CheckToggle {
                component_id,
                checked,
            } => {
                let mut action = ActionMessage::event("toggle", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context(
                        "checked",
                        DynamicValue::Literal(Value::String(checked.to_string())),
                    );
                if let Some(surface_id) = surface_for(&component_id) {
                    if let Some(binding) = self.data_bindings.get(&surface_id) {
                        action = action.with_context(
                            "dataModel",
                            DynamicValue::Literal(binding.as_value().clone()),
                        );
                    }
                }
                Ok(Some(action))
            }
            UserEvent::SliderChange {
                component_id,
                value,
            } => {
                let mut action = ActionMessage::event("slider_change", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context(
                        "value",
                        DynamicValue::Literal(Value::String(value.to_string())),
                    );
                if let Some(surface_id) = surface_for(&component_id) {
                    if let Some(binding) = self.data_bindings.get(&surface_id) {
                        action = action.with_context(
                            "dataModel",
                            DynamicValue::Literal(binding.as_value().clone()),
                        );
                    }
                }
                Ok(Some(action))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::ComponentId;
    use ratatui::{
        style::{Color, Modifier},
        Terminal,
    };
    use serde_json::json;

    #[test]
    fn test_tui_renderer_new() {
        let renderer = TuiRenderer::new();
        assert!(renderer.surfaces.is_empty());
    }

    #[test]
    fn test_create_surface() {
        let renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let _msg = CreateSurface {
            surface_id: "s1".to_string(),
            catalog_id: "a2ui://catalogs/basic/v1".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };

        // 结构验证
        assert!(renderer.surfaces.is_empty());
    }

    #[test]
    fn test_delete_surface_removes_bindings() {
        let renderer = TuiRenderer::new();
        // 结构验证
        assert!(renderer.data_bindings.is_empty());
    }

    #[tokio::test]
    async fn test_dependency_graph_populated_on_create_surface() {
        let mut renderer = TuiRenderer::new();
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path {
                path: "/user/count".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp_a, comp_b]),
                data_model: None,
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        assert!(graph
            .get_dependencies(&ComponentId::new("name_label").unwrap())
            .unwrap()
            .contains("/user/name"));
        assert!(graph
            .get_dependencies(&ComponentId::new("count_label").unwrap())
            .unwrap()
            .contains("/user/count"));
    }

    #[tokio::test]
    async fn test_dependency_graph_isolates_affected_components() {
        let mut renderer = TuiRenderer::new();
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path {
                path: "/user/count".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp_a, comp_b]),
                data_model: None,
            })
            .await
            .unwrap();

        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Alice")),
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        let affected = graph.dependents("/user/name");
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].as_str(), "name_label");
    }

    #[tokio::test]
    async fn test_render_frame_produces_widgets() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Hello".into()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        assert!(buf.area().width > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_column_layout() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Title".into()),
        );
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("title").unwrap()],
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, title]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        assert!(buf.area().width > 0);
        assert!(buf.area().height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_applies_styled_text_degraded_tui_style() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp: Component = serde_json::from_value(json!({
            "id": "root",
            "component": "Text",
            "text": "Styled",
            "style": {
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let cell = terminal.backend().buffer().get(0, 0);
        assert_eq!(cell.symbol(), "S");
        assert_eq!(cell.fg, Color::Rgb(17, 34, 51));
        assert_eq!(cell.bg, Color::Reset);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[tokio::test]
    async fn test_render_frame_applies_styled_icon_degraded_tui_style() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp: Component = serde_json::from_value(json!({
            "id": "root",
            "component": "Icon",
            "name": "star",
            "style": {
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let cell = terminal.backend().buffer().get(0, 0);
        assert_eq!(cell.symbol(), "★");
        assert_eq!(cell.fg, Color::Rgb(17, 34, 51));
        assert_eq!(cell.bg, Color::Reset);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    // --- P0-3: Surface/component limit tests ---

    #[tokio::test]
    async fn test_surface_limit_enforced() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );

        // 创建 100 个 Surface（达到上限）
        for i in 0..100 {
            renderer
                .create_surface(CreateSurface {
                    surface_id: format!("s{}", i),
                    catalog_id: "a2ui://catalogs/basic/v1".into(),
                    surface_properties: None,
                    send_data_model: false,
                    components: Some(vec![comp.clone()]),
                    data_model: None,
                })
                .await
                .unwrap();
        }

        // 第 101 个应被拒绝
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "overflow".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("limit exceeded"));
        }
    }

    #[tokio::test]
    async fn test_component_limit_enforced() {
        let mut renderer = TuiRenderer::new();
        // 创建 1001 个组件（超过上限 1000）
        let components: Vec<_> = (0..1001)
            .map(|i| {
                Component::text(
                    ComponentId::new(format!("c{}", i)).unwrap(),
                    DynamicValue::Literal(format!("text {}", i)),
                )
            })
            .collect();

        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("component limit exceeded"));
        }
    }

    #[tokio::test]
    async fn test_component_limit_allows_under_max() {
        let mut renderer = TuiRenderer::new();
        let components: Vec<_> = (0..100)
            .map(|i| {
                Component::text(
                    ComponentId::new(format!("c{}", i)).unwrap(),
                    DynamicValue::Literal(format!("text {}", i)),
                )
            })
            .collect();

        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    // --- P0-4: callableFrom enforcement tests ---

    #[tokio::test]
    async fn test_call_function_client_only_allowed() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("validate", a2ui_renderer::CallableFrom::ClientOnly);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "validate".into(),
                    args: serde_json::json!({"value": "test"}),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_function_remote_only_rejected() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("fetch", a2ui_renderer::CallableFrom::RemoteOnly);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "fetch".into(),
                    args: serde_json::json!({"url": "https://example.com"}),
                },
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("invalid function call"));
        }
    }

    #[tokio::test]
    async fn test_call_function_unregistered_rejected() {
        let mut renderer = TuiRenderer::new();

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "unknown".into(),
                    args: serde_json::json!({}),
                },
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("not available"));
        }
    }

    #[tokio::test]
    async fn test_call_function_client_or_remote_allowed() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "formatString".into(),
                    args: serde_json::json!({"template": "Hello"}),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_rejects_unregistered() {
        let mut renderer = TuiRenderer::new();
        // 注册一个 Catalog
        let catalog = a2ui_core::Catalog::new("my-catalog".to_string());
        renderer.register_catalog(catalog).unwrap();

        // 使用未注册的 catalogId 创建 Surface → 应被拒绝
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "unknown-catalog".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("not found"));
        }
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_accepts_registered() {
        let mut renderer = TuiRenderer::new();
        let catalog = a2ui_core::Catalog::new("basic".to_string());
        renderer.register_catalog(catalog).unwrap();

        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_accepts_registered_catalog() {
        let mut renderer = TuiRenderer::new();
        // Basic Catalog 已自动注册 → 使用已注册的 catalogId 应成功
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    // --- P1-1: @index scope system — integration tests ---

    #[tokio::test]
    async fn test_template_expansion_integration() {
        let mut renderer = TuiRenderer::new();

        // Template: Text with relative path
        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );

        // Parent with ChildList::Object
        let parent: Component =
            serde_json::from_value(serde_json::json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![parent, template]),
                data_model: Some(serde_json::json!({"items": [{"name": "a"}, {"name": "b"}]})),
            })
            .await
            .unwrap();

        // 验证展开的组件保留了路径绑定（相对路径被转为绝对路径）
        let comp0 = renderer
            .forest
            .get("s1", &ComponentId::new("item_tmpl_0").unwrap());
        assert!(comp0.is_some());
        assert_eq!(
            comp0.unwrap().properties().get("text"),
            Some(&serde_json::json!({"path": "/items/0/name"}))
        );
    }

    #[tokio::test]
    async fn test_template_expansion_with_at_index_integration() {
        let mut renderer = TuiRenderer::new();

        // Template using @index
        let template = Component::text(
            ComponentId::new("idx_tmpl").unwrap(),
            DynamicValue::FunctionCall {
                call: "@index".into(),
                args: serde_json::json!({}),
            },
        );

        let parent: Component =
            serde_json::from_value(serde_json::json!({"component": "Column", "id": "list", "children": {"template": "idx_tmpl", "path": "/items"}}))
                .unwrap();

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![parent, template]),
                data_model: Some(serde_json::json!({"items": [1, 2, 3]})),
            })
            .await
            .unwrap();

        let comp1 = renderer
            .forest
            .get("s1", &ComponentId::new("idx_tmpl_1").unwrap());
        assert!(comp1.is_some());
        assert_eq!(
            comp1.unwrap().properties().get("text"),
            Some(&serde_json::json!(1))
        );
    }

    // --- P1-2: responsePath 写回 ---

    #[tokio::test]
    async fn test_action_response_writes_back_to_datamodel() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 注册 pending response
        renderer.register_pending_response("action-1", "/result");

        // 模拟服务器响应
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    serde_json::json!("done"),
                ),
            })
            .await
            .unwrap();

        // 验证 DataModel 已被更新
        let binding = renderer.data_bindings.get("s1").unwrap();
        assert_eq!(binding.get("/result"), Some(&serde_json::json!("done")));
    }

    #[tokio::test]
    async fn test_action_response_writes_error_to_datamodel() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"error": null})),
            })
            .await
            .unwrap();

        renderer.register_pending_response("action-2", "/error");

        renderer
            .action_response(ActionResponse {
                action_id: "action-2".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Error(
                    a2ui_core::message::server_to_client::ResponseError {
                        code: "TIMEOUT".into(),
                        message: "request timed out".into(),
                    },
                ),
            })
            .await
            .unwrap();

        let binding = renderer.data_bindings.get("s1").unwrap();
        assert_eq!(
            binding.get("/error"),
            Some(&serde_json::json!("request timed out"))
        );
    }

    #[tokio::test]
    async fn test_action_response_unknown_action_id_is_noop() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 未注册的 action_id → 不应崩溃
        let result = renderer
            .action_response(ActionResponse {
                action_id: "unknown-action".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    serde_json::json!("done"),
                ),
            })
            .await;
        assert!(result.is_ok());

        // DataModel 应未被修改
        let binding = renderer.data_bindings.get("s1").unwrap();
        assert_eq!(binding.get("/result"), Some(&serde_json::json!("pending")));
    }

    // --- P2-2: sendDataModel targeting ---

    #[tokio::test]
    async fn test_send_data_model_includes_datamodel_in_action() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        let result = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();

        let action = result.unwrap();
        let data_model_ctx = action.context.get("dataModel");
        assert!(data_model_ctx.is_some());
        let dm_value = data_model_ctx.unwrap().clone();
        if let DynamicValue::Literal(v) = dm_value {
            assert_eq!(v, json!({"user": {"name": "Alice"}}));
        } else {
            panic!("dataModel context should be Literal");
        }
    }

    #[tokio::test]
    async fn test_send_data_model_false_excludes_datamodel() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"secret": "data"})),
            })
            .await
            .unwrap();

        let result = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();

        let action = result.unwrap();
        assert!(action.context.get("dataModel").is_none());
    }

    #[tokio::test]
    async fn test_send_data_model_includes_latest_state() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"count": 0})),
            })
            .await
            .unwrap();

        // 更新 DataModel
        renderer
            .update_data_model(a2ui_core::message::server_to_client::UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/count".into()),
                value: Some(json!(5)),
            })
            .await
            .unwrap();

        // 触发 action 时应包含最新 DataModel
        let result = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();

        let action = result.unwrap();
        let dm_ctx = action.context.get("dataModel").unwrap();
        if let DynamicValue::Literal(v) = dm_ctx {
            assert_eq!(v.get("count"), Some(&json!(5)));
        } else {
            panic!("dataModel context should be Literal");
        }
    }

    // --- P3-3: TextField/CheckBox/Slider rendering tests ---

    #[tokio::test]
    async fn test_render_frame_with_text_field() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let tf: Component = serde_json::from_str(
            r#"{"id":"name_input","component":"TextField","value":"Alice","placeholder":"Enter name"}"#
        ).unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![tf]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_checkbox() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let cb: Component = serde_json::from_str(
            r#"{"id":"agree","component":"CheckBox","checked":true,"label":"I agree"}"#,
        )
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![cb]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_slider() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let sl: Component = serde_json::from_str(
            r#"{"id":"volume","component":"Slider","value":50,"min":0,"max":100}"#,
        )
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![sl]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    // --- CustomComponentRegistry tests ---

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = TuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        assert!(renderer.custom_registry.is_registered("MyChart"));
    }

    #[test]
    fn test_custom_component_registry_duplicate_fails() {
        let mut renderer = TuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let result =
            renderer.register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
    }

    // --- P4-1: Incremental rendering with DependencyGraph ---

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_update_data_model() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // 初始状态：没有脏 surface
        assert!(renderer.dirty_surfaces.is_empty());

        // 更新有依赖的路径 → 应标记 s1 为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Bob")),
            })
            .await
            .unwrap();

        assert!(renderer.dirty_surfaces.contains("s1"));
    }

    #[tokio::test]
    async fn test_incremental_render_no_dirty_on_unbound_path() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // 更新没有组件依赖的路径 → 不应标记为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/other/path".into()),
                value: Some(json!("value")),
            })
            .await
            .unwrap();

        assert!(renderer.dirty_surfaces.is_empty());
    }

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_action_response() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("result_label").unwrap(),
            DynamicValue::Path {
                path: "/result".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 注册 pending response
        renderer.register_pending_response("action-1", "/result");

        // 模拟服务器响应
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    serde_json::json!("done"),
                ),
            })
            .await
            .unwrap();

        // 应标记 s1 为脏（因为 /result 有组件依赖）
        assert!(renderer.dirty_surfaces.contains("s1"));
    }

    #[tokio::test]
    async fn test_incremental_render_clears_dirty_after_render() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // 先标记为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Bob")),
            })
            .await
            .unwrap();
        assert!(renderer.dirty_surfaces.contains("s1"));

        // render_frame 后应清除脏标记
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        assert!(renderer.dirty_surfaces.is_empty());
    }

    #[tokio::test]
    async fn test_incremental_render_full_render_when_no_dirty() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // 没有脏 surface 时，render_frame 应正常渲染所有 surface
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let result = renderer.render_frame(&mut terminal).await;
        assert!(result.is_ok());
        assert!(renderer.dirty_surfaces.is_empty());
    }

    // ---- render() 帧准备测试 ----

    #[tokio::test]
    async fn test_render_builds_widgets_for_valid_surface() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let result = renderer.render().await;
        assert!(result.is_ok());
        // render() 应构建了至少 1 个 widget（root 组件）
        assert!(renderer.last_frame_widget_count > 0);
    }

    #[tokio::test]
    async fn test_render_handles_empty_state() {
        let mut renderer = TuiRenderer::new();
        let result = renderer.render().await;
        assert!(result.is_ok());
        // 无 surface 时 widget 数为 0
        assert_eq!(renderer.last_frame_widget_count, 0);
    }

    // ---- FocusManager integration tests ----

    #[tokio::test]
    async fn test_focus_manager_collects_focusable_after_render() {
        let mut renderer = TuiRenderer::new();
        let btn: Component = serde_json::from_str(
            r#"{"id":"btn","component":"Button","child":"lbl","text":"Click"}"#,
        )
        .unwrap();
        let tf: Component = serde_json::from_str(
            r#"{"id":"tf","component":"TextField","value":"","placeholder":"Enter"}"#,
        )
        .unwrap();
        let root: Component =
            serde_json::from_str(r#"{"id":"root","component":"Column","children":["btn","tf"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, btn, tf]),
                data_model: None,
            })
            .await
            .unwrap();

        renderer.render().await.unwrap();
        // 焦点管理器应收集了 2 个可聚焦组件（Button + TextField）
        assert!(renderer.focus_manager.focusable_count() >= 1);
    }

    #[tokio::test]
    async fn test_tab_key_navigates_focus() {
        let mut renderer = TuiRenderer::new();
        let btn: Component =
            serde_json::from_str(r#"{"id":"btn","component":"Button","child":"lbl","text":"OK"}"#)
                .unwrap();
        let root: Component =
            serde_json::from_str(r#"{"id":"root","component":"Column","children":["btn"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, btn]),
                data_model: None,
            })
            .await
            .unwrap();
        renderer.render().await.unwrap();

        // Tab 键应切换焦点
        let result = renderer
            .handle_user_event(UserEvent::KeyPress { key: "Tab".into() })
            .await;
        assert!(result.is_ok());
    }
}
