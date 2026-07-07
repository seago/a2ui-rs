use crate::html_builder::{HtmlBuilder, RenderableHtmlWidget};
use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_renderer::{
    resolve_dynamic_string_prop, CatalogRegistry, ComponentForest, ComponentStyle,
    CustomComponentRegistry, DataBinding, DependencyGraph, FunctionDispatcher, PathResolver,
    RenderResult, Renderer, SurfaceHandle, SurfaceLru, UserEvent,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Web 渲染器实现
///
/// 将 A2UI 组件树渲染为 HTML 字符串。支持服务端渲染模式，
/// 通过 `render_surface_html()` 和 `render_all_html()` 方法输出 HTML。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer_web::WebRenderer;
///
/// let renderer = WebRenderer::new();
/// assert!(renderer.surfaces.is_empty());
/// ```
#[derive(Debug)]
pub struct WebRenderer {
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
    /// 当前聚焦的组件
    focused_component: Option<ComponentId>,
    /// action_id → response_path 映射（responsePath 写回用）
    pending_responses: HashMap<String, String>,
    /// Surface 的 sendDataModel 标记（为 true 时 action 附带完整 data model）
    send_data_model: HashMap<String, bool>,
    /// 需要增量重渲染的 surface 集合
    dirty_surfaces: HashSet<String>,
    /// Surface LRU 驱逐管理器
    surface_lru: SurfaceLru,
    /// 自定义组件注册表
    custom_registry: CustomComponentRegistry,
    /// 最近一次渲染的 HTML 输出（surface_id → HTML body）
    last_html: HashMap<String, String>,
    /// HTML 构建器
    html_builder: HtmlBuilder,
}

/// 最大并发 Surface 数量（DoS 防护）
const MAX_SURFACES: usize = 100;
/// 单 Surface 最大组件数量（DoS 防护）
const MAX_COMPONENTS_PER_SURFACE: usize = 1000;

impl WebRenderer {
    /// 创建新的 Web 渲染器
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            forest: ComponentForest::new(),
            data_bindings: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            dispatcher: FunctionDispatcher::new(),
            catalog_registry: CatalogRegistry::with_defaults(),
            focused_component: None,
            pending_responses: HashMap::new(),
            send_data_model: HashMap::new(),
            dirty_surfaces: HashSet::new(),
            surface_lru: SurfaceLru::new(MAX_SURFACES, Some(Duration::from_secs(600))),
            custom_registry: CustomComponentRegistry::new(),
            last_html: HashMap::new(),
            html_builder: HtmlBuilder::new(),
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

    /// 注册待响应的 action_id → response_path 映射
    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.pending_responses
            .insert(action_id.into(), response_path.into());
    }

    /// 渲染指定 Surface 为 HTML 字符串
    ///
    /// 返回完整的 HTML body 内容，不包含页面包装（`<html>`、`<head>` 等）。
    /// 使用 `render_page()` 可以包装为完整 HTML 页面。
    pub fn render_surface_html(&self, surface_id: &str) -> Option<String> {
        // 如果有缓存的 HTML 且 surface 不在脏集合中，直接返回缓存
        if !self.dirty_surfaces.contains(surface_id) {
            if let Some(cached) = self.last_html.get(surface_id) {
                return Some(cached.clone());
            }
        }

        // 构建组件树
        let tree = self.forest.build_tree(surface_id).ok()?;

        // 获取 data binding
        let binding = self.data_bindings.get(surface_id)?;

        // 将组件树映射为 RenderableHtmlWidget 树
        let root_widget = Self::build_widget_tree(&tree, binding, &self.custom_registry)?;

        // 渲染为 HTML
        Some(self.html_builder.render(&root_widget))
    }

    /// 渲染所有 Surface 为完整 HTML 页面
    ///
    /// 将所有 Surface 的 HTML body 拼接后嵌入完整页面模板。
    pub fn render_all_html(&self) -> String {
        let all_surface_ids: Vec<String> = self.surfaces.values().cloned().collect();
        let mut bodies = String::new();

        for surface_id in &all_surface_ids {
            if let Some(html) = self.render_surface_html(surface_id) {
                bodies.push_str(&format!(
                    "<div class=\"a2ui-surface\" data-surface-id=\"{}\">",
                    html_attr(surface_id)
                ));
                bodies.push_str(&html);
                bodies.push_str("</div>");
            }
        }

        self.html_builder.render_page(&bodies, "A2UI")
    }

    /// 递归构建 RenderableHtmlWidget 树
    fn build_widget_tree(
        node: &a2ui_renderer::component_forest::ComponentTreeNode,
        binding: &DataBinding,
        registry: &CustomComponentRegistry,
    ) -> Option<RenderableHtmlWidget> {
        let component = &node.component;
        let ctype = component.component_type();
        let props = component.properties();

        // 递归构建子 widget，保留源组件 id 以便 Modal/Tabs 按 id 匹配
        // （组件树 children 的顺序是构建细节，不可按位置消费）
        let mut child_pairs: Vec<(String, RenderableHtmlWidget)> = Vec::new();
        for child in &node.children {
            if let Some(widget) = Self::build_widget_tree(child, binding, registry) {
                child_pairs.push((child.component.id().as_str().to_string(), widget));
            }
        }

        let widget = match ctype {
            "Text" => {
                let text = extract_text(props, binding);
                let variant = props
                    .get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("body")
                    .to_string();
                RenderableHtmlWidget::Text {
                    id: component.id().clone(),
                    text,
                    variant,
                }
            }
            "Button" => {
                let label = extract_text(props, binding);
                let variant = props
                    .get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();
                RenderableHtmlWidget::Button {
                    id: component.id().clone(),
                    label,
                    variant,
                }
            }
            "Column" => RenderableHtmlWidget::Column {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Row" => RenderableHtmlWidget::Row {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Image" => {
                let url = extract_string_value(props, "url", binding).unwrap_or_default();
                RenderableHtmlWidget::Image {
                    id: component.id().clone(),
                    url,
                }
            }
            "Card" => {
                let child = child_pairs.into_iter().next().map(|(_, w)| w).unwrap_or(
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: "empty card".to_string(),
                    },
                );
                RenderableHtmlWidget::Card {
                    id: component.id().clone(),
                    child: Box::new(child),
                }
            }
            "CheckBox" => {
                let checked = resolve_bool_value(props, "value", binding)
                    .or_else(|| resolve_bool_value(props, "checked", binding))
                    .unwrap_or(false);
                let label = extract_string_value(props, "label", binding).unwrap_or_default();
                RenderableHtmlWidget::CheckBox {
                    id: component.id().clone(),
                    checked,
                    label,
                }
            }
            "Divider" => RenderableHtmlWidget::Divider {
                id: component.id().clone(),
            },
            "Icon" => {
                let name = extract_string_value(props, "name", binding).unwrap_or("?".to_string());
                RenderableHtmlWidget::Icon {
                    id: component.id().clone(),
                    name,
                }
            }
            "List" => RenderableHtmlWidget::List {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Tabs" => {
                // 按 tabs[].child 的 id 从 child_pairs 中匹配各 tab 内容
                let mut remaining = child_pairs;
                let tabs_data: Vec<(String, Vec<RenderableHtmlWidget>)> = props
                    .get("tabs")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|tab| {
                                let title = tab.get("title")?.as_str()?.to_string();
                                let children = tab
                                    .get("child")
                                    .and_then(|v| v.as_str())
                                    .and_then(|child_id| {
                                        remaining
                                            .iter()
                                            .position(|(id, _)| id == child_id)
                                            .map(|pos| vec![remaining.remove(pos).1])
                                    })
                                    .unwrap_or_default();
                                Some((title, children))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // 如果从 props 读取到了 tabs 数据，使用该数据
                if !tabs_data.is_empty() {
                    RenderableHtmlWidget::Tabs {
                        id: component.id().clone(),
                        tabs: tabs_data,
                    }
                } else {
                    // 否则使用全部子 widget 作为单一 tab（无元数据的降级路径）
                    RenderableHtmlWidget::Tabs {
                        id: component.id().clone(),
                        tabs: vec![(
                            "Tab".to_string(),
                            remaining.into_iter().map(|(_, w)| w).collect(),
                        )],
                    }
                }
            }
            "Modal" => {
                let title = extract_string_value(props, "title", binding)
                    .or_else(|| extract_string_value(props, "label", binding))
                    .unwrap_or_default();
                // 按 props.content 的 id 匹配（children 可能同时含 trigger）；
                // 无 content 元数据时降级取第一个非 trigger 的子 widget
                let trigger_id = props.get("trigger").and_then(|v| v.as_str());
                let content_widget = match props.get("content").and_then(|v| v.as_str()) {
                    Some(content_id) => child_pairs
                        .into_iter()
                        .find(|(id, _)| id == content_id)
                        .map(|(_, w)| w),
                    None => child_pairs
                        .into_iter()
                        .find(|(id, _)| Some(id.as_str()) != trigger_id)
                        .map(|(_, w)| w),
                };
                let content = content_widget.unwrap_or(RenderableHtmlWidget::Placeholder {
                    id: component.id().clone(),
                    reason: "empty modal".to_string(),
                });
                RenderableHtmlWidget::Modal {
                    id: component.id().clone(),
                    title,
                    content: Box::new(content),
                }
            }
            "Slider" => {
                let value = resolve_number_value(props, "value", binding).unwrap_or(0.0);
                let min = resolve_number_value(props, "min", binding).unwrap_or(0.0);
                let max = resolve_number_value(props, "max", binding).unwrap_or(100.0);
                RenderableHtmlWidget::Slider {
                    id: component.id().clone(),
                    value,
                    min,
                    max,
                }
            }
            "TextField" => {
                let value = extract_string_value(props, "value", binding).unwrap_or_default();
                let placeholder =
                    extract_string_value(props, "placeholder", binding).unwrap_or_default();
                RenderableHtmlWidget::TextField {
                    id: component.id().clone(),
                    value,
                    placeholder,
                }
            }
            "ChoicePicker" => {
                let options = props
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(extract_static_string).collect())
                    .unwrap_or_default();
                let selected = props
                    .get("value")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(extract_static_string).collect())
                    .unwrap_or_default();
                RenderableHtmlWidget::ChoicePicker {
                    id: component.id().clone(),
                    options,
                    selected,
                }
            }
            "DateTimeInput" => {
                let label = extract_string_value(props, "label", binding)
                    .unwrap_or("Select date/time".to_string());
                RenderableHtmlWidget::DateTimeInput {
                    id: component.id().clone(),
                    label,
                }
            }
            "Video" => {
                let url = extract_string_value(props, "url", binding).unwrap_or_default();
                RenderableHtmlWidget::Video {
                    id: component.id().clone(),
                    url,
                }
            }
            "AudioPlayer" => {
                let url = extract_string_value(props, "url", binding).unwrap_or_default();
                RenderableHtmlWidget::AudioPlayer {
                    id: component.id().clone(),
                    url,
                }
            }
            _ => {
                // 先检查自定义组件注册表
                if registry.is_registered(ctype) {
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("custom component: {}", ctype),
                    }
                } else {
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("unknown component type: {}", ctype),
                    }
                }
            }
        };

        let style = ComponentStyle::from_component_props(props);
        let widget = if supports_web_style(ctype) && style != ComponentStyle::default() {
            RenderableHtmlWidget::Styled {
                widget: Box::new(widget),
                style,
            }
        } else {
            widget
        };

        Some(widget)
    }

    /// 执行渲染：将脏 surface 渲染为 HTML 并缓存结果
    pub fn render_to_html(&mut self) -> RenderResult<()> {
        let surfaces_to_render: Vec<_> = if self.dirty_surfaces.is_empty() {
            self.surfaces.values().cloned().collect()
        } else {
            self.dirty_surfaces.iter().cloned().collect()
        };

        for surface_id in &surfaces_to_render {
            if let Some(html) = self.render_surface_html(surface_id) {
                self.last_html.insert(surface_id.clone(), html);
            }
        }

        self.dirty_surfaces.clear();
        Ok(())
    }
}

impl Default for WebRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for WebRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        // LRU 驱逐：检查是否需要驱逐最久未用的 surface
        if let Some(victim_id) = self.surface_lru.find_victim(self.surfaces.len()) {
            self.forest.remove_surface(&victim_id).ok();
            self.data_bindings.remove(&victim_id);
            self.surfaces.retain(|_, sid| sid != &victim_id);
            self.dirty_surfaces.remove(&victim_id);
            self.send_data_model.remove(&victim_id);
            self.last_html.remove(&victim_id);
            self.surface_lru.remove(&victim_id);
        }

        //  enforcing surface limit
        if self.surfaces.len() >= MAX_SURFACES {
            return Err(a2ui_renderer::error::RendererError::SurfaceLimitExceeded {
                current: self.surfaces.len(),
                max: MAX_SURFACES,
            });
        }

        // Catalog 信任链 — catalogId 校验
        if !self.catalog_registry.registered_ids().is_empty()
            && !self.catalog_registry.has_catalog(&msg.catalog_id)
        {
            return Err(a2ui_renderer::error::RendererError::CatalogNotFound(
                msg.catalog_id.clone(),
            ));
        }

        let handle = SurfaceHandle::new();
        let surface_id = msg.surface_id.clone();

        // 注册组件
        if let Some(components) = msg.components.clone() {
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

        // 记录 sendDataModel 标记
        self.send_data_model
            .insert(surface_id.clone(), msg.send_data_model);

        // 展开 ChildList::Object 模板
        if let Some(binding) = self.data_bindings.get(&surface_id) {
            let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));
            self.forest
                .expand_templates(&surface_id, binding, &resolver, &self.dispatcher)?;
        }

        // 记录 Surface 映射
        self.surfaces.insert(handle, surface_id.clone());
        self.surface_lru.touch(&surface_id);

        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        for comp in msg.components {
            self.forest.upsert(&surface_id, comp)?;
        }
        // 标记需要重新渲染
        self.dirty_surfaces.insert(surface_id);
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        if let Some(binding) = self.data_bindings.get_mut(&surface_id) {
            if let Some(path) = &msg.path {
                binding.set(path, msg.value.unwrap_or(Value::Null))?;
                // 查询依赖图，获取需要重渲染的组件
                let affected = self.dependency_graph.on_data_change(path);
                if !affected.is_empty() {
                    self.dirty_surfaces.insert(surface_id);
                }
            }
        }
        Ok(())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.forest.remove_surface(&surface_id)?;
        self.data_bindings.remove(&surface_id);
        self.dirty_surfaces.remove(&surface_id);
        self.send_data_model.remove(&surface_id);
        self.surfaces.retain(|_, sid| sid != &surface_id);
        self.last_html.remove(&surface_id);
        self.surface_lru.remove(&surface_id);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        let action_id = msg.action_id.clone();
        if let Some(response_path) = self.pending_responses.remove(&action_id) {
            let write_value = match &msg.response {
                a2ui_core::message::server_to_client::ActionResponsePayload::Success(v) => {
                    v.clone()
                }
                a2ui_core::message::server_to_client::ActionResponsePayload::Error(err) => {
                    Value::String(err.message.clone())
                }
            };

            for (surface_id, binding) in self.data_bindings.iter_mut() {
                if binding.as_value().pointer(&response_path).is_some() || response_path == "/" {
                    self.surface_lru.touch(surface_id);
                    binding.set(&response_path, write_value)?;
                    let affected = self.dependency_graph.on_data_change(&response_path);
                    if !affected.is_empty() {
                        self.dirty_surfaces.insert(surface_id.clone());
                    }
                    break;
                }
            }

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
        self.render_to_html()
    }

    async fn handle_user_event(&mut self, event: UserEvent) -> RenderResult<Option<ActionMessage>> {
        // 先把输入值写回组件声明的绑定路径（在读取 dataModel 快照之前），
        // 使快照与后续渲染反映最新输入
        if let Some((surface_id, path)) =
            a2ui_renderer::write_back_user_event(&self.forest, &mut self.data_bindings, &event)?
        {
            self.surface_lru.touch(&surface_id);
            self.dependency_graph.on_data_change(&path);
            self.dirty_surfaces.insert(surface_id);
        }

        let send_data_surface = self
            .send_data_model
            .iter()
            .find(|(_, &enabled)| enabled)
            .map(|(sid, _)| sid.clone());

        match event {
            UserEvent::Click { component_id } => {
                let mut action = ActionMessage::event("click", "").with_context(
                    "source",
                    DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                );
                if let Some(ref surface_id) = send_data_surface {
                    if let Some(binding) = self.data_bindings.get(surface_id) {
                        action = action.with_context(
                            "dataModel",
                            DynamicValue::Literal(binding.as_value().clone()),
                        );
                    }
                }
                Ok(Some(action))
            }
            UserEvent::KeyPress { key } => {
                if key == "Enter" || key == " " {
                    if let Some(ref comp_id) = self.focused_component {
                        let mut action = ActionMessage::event("activate", "").with_context(
                            "source",
                            DynamicValue::Literal(Value::String(comp_id.as_str().to_string())),
                        );
                        if let Some(ref surface_id) = send_data_surface {
                            if let Some(binding) = self.data_bindings.get(surface_id) {
                                action = action.with_context(
                                    "dataModel",
                                    DynamicValue::Literal(binding.as_value().clone()),
                                );
                            }
                        }
                        return Ok(Some(action));
                    }
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
                if let Some(ref surface_id) = send_data_surface {
                    if let Some(binding) = self.data_bindings.get(surface_id) {
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
                if let Some(ref surface_id) = send_data_surface {
                    if let Some(binding) = self.data_bindings.get(surface_id) {
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
                if let Some(ref surface_id) = send_data_surface {
                    if let Some(binding) = self.data_bindings.get(surface_id) {
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

/// 从组件 properties 中提取文本内容，支持 DynamicValue::Path 解析
fn extract_text(props: &Value, binding: &DataBinding) -> String {
    resolve_dynamic_string_prop(props, "text", Some(binding), "")
}

/// 从组件 properties 中提取字符串类型的值，支持 DynamicValue::Path 解析
fn extract_string_value(props: &Value, key: &str, binding: &DataBinding) -> Option<String> {
    props
        .get(key)
        .map(|_| resolve_dynamic_string_prop(props, key, Some(binding), ""))
}

fn resolve_bool_value(props: &Value, key: &str, binding: &DataBinding) -> Option<bool> {
    let value = props.get(key)?;
    if let Some(value) = value.as_bool() {
        return Some(value);
    }
    let path = value
        .as_object()
        .and_then(|obj| obj.get("path"))
        .and_then(|v| v.as_str())?;
    binding.get(path).and_then(|value| value.as_bool())
}

fn resolve_number_value(props: &Value, key: &str, binding: &DataBinding) -> Option<f64> {
    let value = props.get(key)?;
    if let Some(value) = value.as_f64() {
        return Some(value);
    }
    let path = value
        .as_object()
        .and_then(|obj| obj.get("path"))
        .and_then(|v| v.as_str())?;
    binding.get(path).and_then(|value| value.as_f64())
}

/// 从 JSON 值中提取静态字符串（非 DynamicValue）
fn extract_static_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn supports_web_style(component_type: &str) -> bool {
    matches!(
        component_type,
        "Text" | "Icon" | "Row" | "Column" | "List" | "Card" | "Image"
    )
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

/// HTML 属性值转义
fn html_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::ComponentId;
    use serde_json::json;

    #[test]
    fn test_web_renderer_new() {
        let renderer = WebRenderer::new();
        assert!(renderer.surfaces.is_empty());
    }

    #[test]
    fn test_register_function() {
        let mut renderer = WebRenderer::new();
        renderer.register_function("upper", a2ui_renderer::CallableFrom::ClientOrRemote);
        assert!(renderer
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "upper"));
    }

    #[test]
    fn test_register_catalog() {
        let mut renderer = WebRenderer::new();
        let catalog: a2ui_core::Catalog = serde_json::from_value(serde_json::json!({
            "catalogId": "basic",
            "instructions": "Basic catalog",
            "components": {},
            "functions": {}
        }))
        .unwrap();
        assert!(renderer.register_catalog(catalog).is_ok());
    }

    #[tokio::test]
    async fn test_create_and_render_surface() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello World".into()),
        );
        let handle = renderer
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

        assert!(renderer.surfaces.contains_key(&handle));

        let html = renderer.render_surface_html("s1");
        assert!(html.is_some());
        assert!(html.unwrap().contains("Hello World"));
    }

    #[tokio::test]
    async fn test_text_input_writes_back_and_renders_new_value() {
        let mut renderer = WebRenderer::new();
        let field: Component = serde_json::from_value(json!({
            "component":"TextField","id":"root","value":{"path":"/form/username"}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![field]),
                data_model: Some(json!({"form": {"username": "old"}})),
            })
            .await
            .unwrap();

        let action = renderer
            .handle_user_event(UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap()
            .unwrap();

        // 绑定路径已更新
        assert_eq!(
            renderer
                .data_bindings
                .get("s1")
                .unwrap()
                .get("/form/username"),
            Some(&json!("alice"))
        );
        // dataModel 快照含新值
        let Some(DynamicValue::Literal(dm)) = action.context.get("dataModel").cloned() else {
            panic!("dataModel context should be Literal");
        };
        assert_eq!(dm.pointer("/form/username"), Some(&json!("alice")));
        // 写回后渲染的 HTML 含新值（脏标记 → 缓存失效链路）
        let html = renderer.render_surface_html("s1").expect("html");
        assert!(html.contains("alice"), "HTML 应含写回后的新值: {html}");
    }

    #[tokio::test]
    async fn test_check_toggle_writes_back_to_data_model() {
        let mut renderer = WebRenderer::new();
        let checkbox: Component = serde_json::from_value(json!({
            "component":"CheckBox","id":"root","checked":{"path":"/agree"}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![checkbox]),
                data_model: Some(json!({"agree": false})),
            })
            .await
            .unwrap();

        renderer
            .handle_user_event(UserEvent::CheckToggle {
                component_id: ComponentId::new("root").unwrap(),
                checked: true,
            })
            .await
            .unwrap();

        assert_eq!(
            renderer.data_bindings.get("s1").unwrap().get("/agree"),
            Some(&json!(true))
        );
        assert!(renderer.dirty_surfaces.contains("s1"));
    }

    #[tokio::test]
    async fn test_modal_widget_gets_content_by_id() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            serde_json::from_value(json!({
                "id":"root","component":"Modal",
                "title":"Confirm","content":"body","trigger":"btn"
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id":"body","component":"Text","text":"HELLO_MODAL"
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id":"btn","component":"Button","label":"open"
            }))
            .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("HELLO_MODAL"),
            "Modal content 组件应被渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_tabs_widget_populates_tab_children() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            serde_json::from_value(json!({
                "id":"root","component":"Tabs",
                "tabs":[{"title":"First","child":"a"},{"title":"Second","child":"b"}]
            }))
            .unwrap(),
            serde_json::from_value(json!({"id":"a","component":"Text","text":"TAB_A_TEXT"}))
                .unwrap(),
            serde_json::from_value(json!({"id":"b","component":"Text","text":"TAB_B_TEXT"}))
                .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("TAB_A_TEXT"),
            "tab 0 内容应被渲染，got: {html}"
        );
        assert!(
            html.contains("TAB_B_TEXT"),
            "tab 1 内容应被渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_modal_without_content_does_not_render_trigger_as_body() {
        // Modal 只有 trigger 无 content：按位置取第一个子 widget 会把
        // trigger 误当 content 塞进 modal body——必须按 id 匹配
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            serde_json::from_value(json!({
                "id":"root","component":"Modal","title":"T","trigger":"btn"
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id":"btn","component":"Button","label":"TRIGGER_LABEL"
            }))
            .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        // trigger 不应出现在 modal body 中
        let body_start = html.find("a2ui-modal-body").expect("modal body present");
        assert!(
            !html[body_start..].contains("TRIGGER_LABEL"),
            "trigger 不应被当作 modal content 渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_dynamic_string_props_render_from_data_model() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            serde_json::from_value(json!({
                "id": "root",
                "component": "Column",
                "children": ["title", "text_field", "checkbox", "slider", "icon"]
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "title",
                "component": "Text",
                "text": {"path": "/title"}
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "text_field",
                "component": "TextField",
                "value": {"path": "/form/username"},
                "placeholder": {"path": "/form/placeholder"}
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "checkbox",
                "component": "CheckBox",
                "label": {"path": "/remember"},
                "value": {"path": "/rememberChecked"}
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "slider",
                "component": "Slider",
                "value": {"path": "/volume"},
                "min": 0,
                "max": 100
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "icon",
                "component": "Icon",
                "name": {"path": "/missing_icon"}
            }))
            .unwrap(),
        ];

        renderer
            .create_surface(CreateSurface {
                surface_id: "dynamic".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: Some(json!({
                    "title": "Welcome",
                    "remember": "记住密码",
                    "rememberChecked": true,
                    "volume": 42.0,
                    "form": {
                        "username": "Alice",
                        "placeholder": "请输入用户名"
                    }
                })),
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("dynamic").unwrap();
        assert!(html.contains("Welcome"));
        assert!(html.contains("Alice"));
        assert!(html.contains("请输入用户名"));
        assert!(html.contains("记住密码"));
        assert!(html.contains("checked"));
        assert!(html.contains("value=\"42\""));
        assert!(html.contains("{path:/missing_icon}"));
    }

    #[tokio::test]
    async fn test_web_renderer_applies_shared_style_contract() {
        let mut renderer = WebRenderer::new();
        let style = json!({
            "fontSize": 18,
            "strong": true,
            "color": "#112233",
            "fill": "#44556680",
            "padding": 9,
            "spacing": {"x": 7, "y": 11},
            "radius": 5
        });
        let components: Vec<Component> = vec![
            serde_json::from_value(json!({
                "id": "root",
                "component": "Column",
                "children": ["title", "icon", "row", "card", "list"],
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "title",
                "component": "Text",
                "text": "Styled",
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "icon",
                "component": "Icon",
                "name": "star",
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "row",
                "component": "Row",
                "children": ["image"],
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "image",
                "component": "Image",
                "url": "https://example.com/image.png",
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "card",
                "component": "Card",
                "child": "card_text",
                "style": style.clone()
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "card_text",
                "component": "Text",
                "text": "Card text"
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "list",
                "component": "List",
                "children": ["list_text"],
                "style": style
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "id": "list_text",
                "component": "Text",
                "text": "List text"
            }))
            .unwrap(),
        ];

        renderer
            .create_surface(CreateSurface {
                surface_id: "styled".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("styled").unwrap();

        assert!(html.contains("font-size:18px"));
        assert!(html.contains("font-weight:700"));
        assert!(html.contains("color:#112233"));
        assert!(html.contains("background-color:rgba(68,85,102,0.502)"));
        assert!(html.contains("padding:9px"));
        assert!(html.contains("border-radius:5px"));
        assert!(html.contains("gap:11px 7px"));
    }

    #[tokio::test]
    async fn test_render_all_html() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
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

        let full_html = renderer.render_all_html();
        assert!(full_html.contains("<!DOCTYPE html>"));
        assert!(full_html.contains("Hello"));
        assert!(full_html.contains("a2ui-surface"));
    }

    #[tokio::test]
    async fn test_dependency_graph_populated() {
        let mut renderer = WebRenderer::new();
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
                data_model: None,
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        let deps = graph
            .get_dependencies(&ComponentId::new("name_label").unwrap())
            .unwrap();
        assert!(deps.contains("/user/name"));
    }

    #[tokio::test]
    async fn test_incremental_render_marks_dirty() {
        let mut renderer = WebRenderer::new();
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

        assert!(renderer.dirty_surfaces.is_empty());

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
    async fn test_render_all_html_with_multiple_surfaces() {
        let mut renderer = WebRenderer::new();
        let comp1 = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Surface 1".into()),
        );
        let comp2 = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Surface 2".into()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp1]),
                data_model: None,
            })
            .await
            .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s2".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp2]),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_all_html();
        assert!(html.contains("Surface 1"));
        assert!(html.contains("Surface 2"));
    }

    // --- Surface/component limit tests ---

    #[tokio::test]
    async fn test_surface_limit_enforced() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );

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
    }

    #[tokio::test]
    async fn test_component_limit_enforced() {
        let mut renderer = WebRenderer::new();
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
    }

    #[tokio::test]
    async fn test_component_limit_allows_under_max() {
        let mut renderer = WebRenderer::new();
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

    // --- CustomComponentRegistry tests ---

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = WebRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        assert!(renderer.custom_registry.is_registered("MyChart"));
    }

    #[test]
    fn test_custom_component_registry_duplicate_fails() {
        let mut renderer = WebRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let result =
            renderer.register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_vs_custom_placeholder_in_html() {
        let mut renderer = WebRenderer::new();

        // 先注册自定义组件
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();

        // 未知组件（不是 root，而是 root 的子组件）
        let unknown_comp: Component =
            serde_json::from_str(r#"{"id":"u1","component":"UnknownType"}"#).unwrap();
        let root_unknown = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("u1").unwrap()],
        );

        // 自定义组件（已注册）
        let custom_comp: Component =
            serde_json::from_str(r#"{"id":"c1","component":"MyChart"}"#).unwrap();
        let root_custom = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("c1").unwrap()],
        );

        // 创建 surface 测试未知组件
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root_unknown, unknown_comp]),
                data_model: None,
            })
            .await
            .unwrap();

        // 创建 surface 测试自定义组件
        renderer
            .create_surface(CreateSurface {
                surface_id: "s2".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root_custom, custom_comp]),
                data_model: None,
            })
            .await
            .unwrap();

        // 未知组件 → "unknown component type"
        let html1 = renderer.render_surface_html("s1").unwrap();
        assert!(html1.contains("unknown component type"));

        // 注册过的自定义组件 → "custom component"
        let html2 = renderer.render_surface_html("s2").unwrap();
        assert!(html2.contains("custom component"));
    }

    // --- callableFrom enforcement tests ---

    #[tokio::test]
    async fn test_call_function_client_only_allowed() {
        let mut renderer = WebRenderer::new();
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
        let mut renderer = WebRenderer::new();
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
    }
}
