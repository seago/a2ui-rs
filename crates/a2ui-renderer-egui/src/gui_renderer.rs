use crate::widget_mapper::RenderableGuiWidget;
use crate::WidgetMapper;
use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{
    CatalogRegistry, ComponentForest, CustomComponentRegistry, DataBinding, DependencyGraph,
    FunctionDispatcher, PathResolver, RenderResult, Renderer, SurfaceHandle, SurfaceLru, UserEvent,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::Duration;

/// egui 示例页面的内容最大宽度
const CONTENT_MAX_WIDTH: f32 = 720.0;

/// 缓存的 egui 纹理（包装 TextureHandle 以支持 Debug）
struct CachedTexture(egui::TextureHandle);

impl std::fmt::Debug for CachedTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedTexture")
            .field("id", &self.0.id())
            .finish()
    }
}

/// GUI 渲染器实现
#[derive(Debug)]
pub struct GuiRenderer {
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
    /// 图片纹理缓存（URL → CachedTexture，需保持存活以免纹理被释放）
    image_cache: HashMap<String, CachedTexture>,
}

/// 最大并发 Surface 数量（DoS 防护）
const MAX_SURFACES: usize = 100;
/// 单 Surface 最大组件数量（DoS 防护）
const MAX_COMPONENTS_PER_SURFACE: usize = 1000;

impl GuiRenderer {
    /// 创建新的 GUI 渲染器
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
            image_cache: HashMap::new(),
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

    /// 加载图片到 egui 纹理缓存（如未缓存）
    /// 返回 (TextureId, [width, height]) 用于渲染
    fn load_image(
        &mut self,
        url: &str,
        ctx: &egui::Context,
    ) -> Option<(egui::TextureId, [usize; 2])> {
        if let Some(cached) = self.image_cache.get(url) {
            let raw = cached.0.size();
            let size = [raw[0] as _, raw[1] as _];
            return Some((cached.0.id(), size));
        }

        let response = ureq::get(url).call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).ok()?;

        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as _, rgba.height() as _);
        let size = [w, h];

        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba.into_raw());
        let handle = ctx.load_texture(url, color_image, egui::TextureOptions::default());
        let tex_id = handle.id();
        self.image_cache
            .insert(url.to_string(), CachedTexture(handle));
        Some((tex_id, size))
    }

    /// 使用 egui 渲染一帧，返回用户交互生成的 action 消息
    /// 支持增量渲染：只重渲染 dirty_surfaces 中的 surface
    pub fn render_frame(
        &mut self,
        ctx: &egui::Context,
    ) -> RenderResult<Vec<a2ui_core::message::client_to_server::ActionMessage>> {
        let mapper = WidgetMapper;
        let mut all_actions = Vec::new();

        // 确定要渲染的 surface 列表
        let surfaces_to_render: Vec<_> = if self.dirty_surfaces.is_empty() {
            self.surfaces.values().cloned().collect()
        } else {
            self.dirty_surfaces.iter().cloned().collect()
        };

        for surface_id in &surfaces_to_render {
            // 使用 build_tree 构建组件树
            let tree = match self.forest.build_tree(surface_id) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // 从组件树构建 flat widget map（传入 data model 用于路径解析）
            let mut widget_map: HashMap<String, RenderableGuiWidget> = HashMap::new();
            let data_model = self.data_bindings.get(surface_id);
            Self::flatten_tree_to_widget_map(
                &tree,
                &mapper,
                &mut widget_map,
                &self.custom_registry,
                data_model,
            );

            // 第二遍：回填 Button 的 label — 从 child Text 组件取文字
            Self::resolve_button_labels(&mut widget_map);

            // 预加载所有 Image 组件引用的图片
            let image_textures: HashMap<String, (egui::TextureId, [usize; 2])> = widget_map
                .iter()
                .filter_map(|(id, w)| {
                    if let RenderableGuiWidget::Image { url, .. } = w {
                        self.load_image(url, ctx).map(|tex| (id.clone(), tex))
                    } else {
                        None
                    }
                })
                .collect();

            // 获取 root 组件并渲染整个树
            if let Some(root_widget) = widget_map.get("root") {
                let root_clone = root_widget.clone();
                let mut response_tracker: HashMap<String, egui::Response> = HashMap::new();
                let mut user_events: Vec<a2ui_renderer::UserEvent> = Vec::new();

                egui::CentralPanel::default().show(ctx, |ui| {
                    let available_width = ui.available_width();
                    let content_width = (available_width * 0.92)
                        .max(320.0)
                        .min(CONTENT_MAX_WIDTH)
                        .min(available_width);
                    let left_pad = ((available_width - content_width) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(left_pad);
                        ui.allocate_ui_with_layout(
                            egui::vec2(content_width, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                mapper.render_gui_widget(
                                    &root_clone,
                                    ui,
                                    &widget_map,
                                    &mut response_tracker,
                                    &mut user_events,
                                    &image_textures,
                                );
                            },
                        );
                    });
                });

                // 处理收集到的用户事件，生成 action 消息
                for event in user_events {
                    if let Ok(Some(action)) = pollster::block_on(self.handle_user_event(event)) {
                        all_actions.push(action);
                    }
                }
            }
        }

        self.dirty_surfaces.clear();
        Ok(all_actions)
    }

    /// 递归遍历组件树节点，构建 flat widget map
    fn flatten_tree_to_widget_map(
        node: &ComponentTreeNode,
        mapper: &WidgetMapper,
        widget_map: &mut HashMap<String, RenderableGuiWidget>,
        registry: &CustomComponentRegistry,
        data_model: Option<&a2ui_renderer::DataBinding>,
    ) {
        let widget = mapper.map_to_gui_widget(&node.component, registry, data_model);
        widget_map.insert(node.component.id().as_str().to_string(), widget);
        for child in &node.children {
            Self::flatten_tree_to_widget_map(child, mapper, widget_map, registry, data_model);
        }
    }

    /// 从 Button 的 child Text 组件中回填 label 文本
    /// A2UI Button 的文字在 child 引用的 Text 组件里，而非自身属性
    fn resolve_button_labels(widget_map: &mut HashMap<String, RenderableGuiWidget>) {
        // 先收集所有 Text widget 的文字
        let text_map: HashMap<String, String> = widget_map
            .iter()
            .filter_map(|(id, w)| {
                if let RenderableGuiWidget::Text { text, .. } = w {
                    Some((id.clone(), text.clone()))
                } else {
                    None
                }
            })
            .collect();

        // 更新所有 Button 的 label
        for widget in widget_map.values_mut() {
            if let RenderableGuiWidget::Button {
                label, child_id, ..
            } = widget
            {
                // 如果 label 是占位符格式，尝试从 child 获取文字
                if label.starts_with('[') || label.is_empty() {
                    if let Some(text) = text_map.get(child_id.as_str()) {
                        *label = text.clone();
                    }
                }
            }
        }
    }
}

impl Default for GuiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for GuiRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
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

        // enforcing surface limit（最后保护）
        if self.surfaces.len() >= MAX_SURFACES {
            return Err(a2ui_renderer::error::RendererError::SurfaceLimitExceeded {
                current: self.surfaces.len(),
                max: MAX_SURFACES,
            });
        }

        // Catalog 信任链 — catalogId 校验
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
            // enforcing component limit
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
                let paths = extract_paths(&comp);
                for path in paths {
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

        // 展开 ChildList::Object 模板（@index 作用域系统）
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
        let surface_id = msg.surface_id.clone();
        self.forest.remove_surface(&surface_id)?;
        self.data_bindings.remove(&surface_id);
        // 移除 surface 映射
        self.surfaces.retain(|_, sid| sid != &surface_id);
        // 移除 LRU 追踪
        self.surface_lru.remove(&surface_id);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        // responsePath 写回 — 根据 action_id 查找 response_path 并写入 DataModel
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
        // 实际渲染由平台 crate 处理
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_renderer_new() {
        let renderer = GuiRenderer::new();
        assert!(renderer.surfaces.is_empty());
    }

    #[test]
    fn test_register_function() {
        let mut renderer = GuiRenderer::new();
        renderer.register_function("upper", a2ui_renderer::CallableFrom::ClientOrRemote);
        assert!(renderer
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "upper"));
    }

    #[test]
    fn test_register_catalog() {
        let mut renderer = GuiRenderer::new();
        let catalog: a2ui_core::Catalog = serde_json::from_value(serde_json::json!({
            "catalogId": "basic",
            "instructions": "Basic catalog",
            "components": {},
            "functions": {}
        }))
        .unwrap();
        assert!(renderer.register_catalog(catalog).is_ok());
    }

    // --- Incremental rendering with DependencyGraph ---

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_update_data_model() {
        let mut renderer = GuiRenderer::new();
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
    async fn test_incremental_render_no_dirty_on_unbound_path() {
        let mut renderer = GuiRenderer::new();
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

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = GuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        // 注册成功，不会 panic
        assert!(renderer.custom_registry.is_registered("MyChart"));
    }

    #[test]
    fn test_custom_component_registry_duplicate_fails() {
        let mut renderer = GuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let result =
            renderer.register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_vs_custom_placeholder() {
        use crate::widget_mapper::WidgetMapper;

        let mapper = WidgetMapper;
        let empty_reg = a2ui_renderer::CustomComponentRegistry::new();

        // 未注册的组件类型 → "unknown component type"
        let unknown: Component =
            serde_json::from_str(r#"{"id":"u1","component":"UnknownType"}"#).unwrap();
        let w = mapper.map_to_gui_widget(&unknown, &empty_reg, None);
        assert!(
            matches!(w, RenderableGuiWidget::Placeholder { ref reason, .. } if reason.contains("unknown"))
        );

        // 注册后的自定义组件 → "custom component"
        let mut custom_reg = a2ui_renderer::CustomComponentRegistry::new();
        custom_reg
            .register(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let custom: Component =
            serde_json::from_str(r#"{"id":"c1","component":"MyChart"}"#).unwrap();
        let w = mapper.map_to_gui_widget(&custom, &custom_reg, None);
        assert!(
            matches!(w, RenderableGuiWidget::Placeholder { ref reason, .. } if reason.contains("custom"))
        );
    }

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_action_response() {
        let mut renderer = GuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("result_label").unwrap(),
            DynamicValue::Path {
                path: "/result".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "basic".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"result": "pending"})),
            })
            .await
            .unwrap();

        renderer.register_pending_response("action-1", "/result");

        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    serde_json::json!("done"),
                ),
            })
            .await
            .unwrap();

        assert!(renderer.dirty_surfaces.contains("s1"));
    }
}
