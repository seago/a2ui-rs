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
    CatalogRegistry, ComponentForest, CustomComponentRegistry, DataBinding, DependencyGraph,
    FunctionDispatcher, PathResolver, RenderResult, Renderer, SurfaceHandle, SurfaceLru, UserEvent,
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
            catalog_registry: CatalogRegistry::new(),
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

        // 递归构建子 widget
        let mut child_widgets: Vec<RenderableHtmlWidget> = Vec::new();
        for child in &node.children {
            if let Some(widget) = Self::build_widget_tree(child, binding, registry) {
                child_widgets.push(widget);
            }
        }

        let widget =
            match ctype {
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
                    children: child_widgets,
                },
                "Row" => RenderableHtmlWidget::Row {
                    id: component.id().clone(),
                    children: child_widgets,
                },
                "Image" => {
                    let url = extract_string_value(props, "url", binding).unwrap_or_default();
                    RenderableHtmlWidget::Image {
                        id: component.id().clone(),
                        url,
                    }
                }
                "Card" => {
                    let child = child_widgets.into_iter().next().unwrap_or(
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
                    let checked = props
                        .get("checked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let checked = if !checked {
                        props
                            .get("value")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    } else {
                        true
                    };
                    let label = extract_text(props, binding);
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
                    let name =
                        extract_string_value(props, "name", binding).unwrap_or("?".to_string());
                    RenderableHtmlWidget::Icon {
                        id: component.id().clone(),
                        name,
                    }
                }
                "List" => RenderableHtmlWidget::List {
                    id: component.id().clone(),
                    children: child_widgets,
                },
                "Tabs" => {
                    // Tabs 子组件已由 build_tree 展开为 children
                    // 尝试从 props 读取 tabs 元数据
                    let tabs_data: Vec<(String, Vec<RenderableHtmlWidget>)> = props
                        .get("tabs")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|tab| {
                                    let title = tab.get("title")?.as_str()?.to_string();
                                    Some((title, Vec::new()))
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
                        // 否则使用 child_widgets 作为 flat children
                        RenderableHtmlWidget::Tabs {
                            id: component.id().clone(),
                            tabs: vec![("Tab".to_string(), child_widgets)],
                        }
                    }
                }
                "Modal" => {
                    let title = extract_string_value(props, "title", binding)
                        .or_else(|| extract_string_value(props, "label", binding))
                        .unwrap_or_default();
                    let content = child_widgets.into_iter().next().unwrap_or(
                        RenderableHtmlWidget::Placeholder {
                            id: component.id().clone(),
                            reason: "empty modal".to_string(),
                        },
                    );
                    RenderableHtmlWidget::Modal {
                        id: component.id().clone(),
                        title,
                        content: Box::new(content),
                    }
                }
                "Slider" => {
                    let value = props.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let min = props.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let max = props.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0);
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
                        .map(|arr| {
                            arr.iter()
                                .filter_map(extract_static_string)
                                .collect()
                        })
                        .unwrap_or_default();
                    let selected = props
                        .get("value")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(extract_static_string)
                                .collect()
                        })
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

        if !self
            .dispatcher
            .can_call_from(&function_name, a2ui_renderer::CallableFrom::ClientOnly)
        {
            if self.dispatcher.get(&function_name).is_some() {
                return Err(a2ui_renderer::error::RendererError::InvalidFunctionCall(
                    function_name,
                ));
            }
            return Err(a2ui_renderer::error::RendererError::FunctionNotAvailable(
                function_name,
            ));
        }

        let result = self.dispatcher.dispatch(&function_name, msg.call.args)?;
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
    if let Some(text_val) = props.get("text") {
        if let Some(s) = text_val.as_str() {
            return s.to_string();
        }
        if let Some(obj) = text_val.as_object() {
            if let Some(path_val) = obj.get("path") {
                if let Some(p) = path_val.as_str() {
                    if let Some(resolved) = binding.get(p) {
                        if let Some(s) = resolved.as_str() {
                            return s.to_string();
                        }
                        return resolved.to_string();
                    }
                    return format!("{{path:{}}}", p);
                }
            }
            if let Some(call_val) = obj.get("call") {
                if let Some(c) = call_val.as_str() {
                    return format!("{{call:{}}}", c);
                }
            }
        }
    }
    String::new()
}

/// 从组件 properties 中提取字符串类型的值，支持 DynamicValue::Path 解析
fn extract_string_value(props: &Value, key: &str, binding: &DataBinding) -> Option<String> {
    let value = props.get(key)?;
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = value.as_object() {
        if let Some(path_val) = obj.get("path") {
            if let Some(p) = path_val.as_str() {
                if let Some(resolved) = binding.get(p) {
                    if let Some(s) = resolved.as_str() {
                        return Some(s.to_string());
                    }
                    return Some(resolved.to_string());
                }
                return Some(format!("{{path:{}}}", p));
            }
        }
        if let Some(call_val) = obj.get("call") {
            if let Some(c) = call_val.as_str() {
                return Some(format!("{{call:{}}}", c));
            }
        }
    }
    None
}

/// 从 JSON 值中提取静态字符串（非 DynamicValue）
fn extract_static_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        _ => None,
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
                catalog_id: "basic".into(),
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
    async fn test_render_all_html() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".into()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                    catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
        let result = renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
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
        let unknown_comp: Component = serde_json::from_str(
            r#"{"id":"u1","component":"UnknownType"}"#,
        )
        .unwrap();
        let root_unknown = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("u1").unwrap()],
        );

        // 自定义组件（已注册）
        let custom_comp: Component = serde_json::from_str(
            r#"{"id":"c1","component":"MyChart"}"#,
        )
        .unwrap();
        let root_custom = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("c1").unwrap()],
        );

        // 创建 surface 测试未知组件
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "basic".into(),
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
                catalog_id: "basic".into(),
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
