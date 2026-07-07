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
    CustomComponentRegistry, RenderResult, Renderer, RendererCore, SurfaceHandle, UserEvent,
};
use std::collections::HashMap;
use std::io::Read;

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
///
/// 协议状态与消息处理全部委托 [`RendererCore`]，本类型只保留平台特有部分：
/// 键盘激活转译（Enter/空格 → 焦点组件 Click）与 egui 图片纹理缓存。
/// egui 无 surface 级渲染缓存，核心返回的 [`a2ui_renderer::CoreEffects`] 被忽略。
#[derive(Debug)]
pub struct GuiRenderer {
    /// 渲染器公共核心（协议状态 + 消息流水线；pub 供同 crate 测试访问）
    pub core: RendererCore,
    /// 当前聚焦的组件（现为死代码：无写入路径，保留待焦点专项）
    focused_component: Option<ComponentId>,
    /// 图片纹理缓存（URL → CachedTexture，按 URL 键控、与 surface 生命周期
    /// 无关，需保持存活以免纹理被释放）
    image_cache: HashMap<String, CachedTexture>,
}

impl GuiRenderer {
    /// 创建新的 GUI 渲染器
    pub fn new() -> Self {
        Self {
            core: RendererCore::new(),
            focused_component: None,
            image_cache: HashMap::new(),
        }
    }

    /// 获取依赖图的只读引用（用于测试和查询）
    pub fn dependency_graph(&self) -> &a2ui_renderer::DependencyGraph {
        self.core.dependency_graph()
    }

    /// 注册客户端函数（供 callableFrom enforcement 使用）
    pub fn register_function(
        &mut self,
        name: impl Into<String>,
        callable_from: a2ui_renderer::CallableFrom,
    ) {
        self.core.register_function(name, callable_from);
    }

    /// 获取已注册函数列表
    pub fn registered_functions(&self) -> Vec<&String> {
        self.core.registered_functions()
    }

    /// 注册 Catalog（用于 catalogId 信任链校验）
    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.core.register_catalog(catalog)
    }

    /// 获取 Catalog 注册表的只读引用
    pub fn catalog_registry(&self) -> &a2ui_renderer::CatalogRegistry {
        self.core.catalog_registry()
    }

    /// 注册自定义组件类型
    pub fn register_custom_component(
        &mut self,
        def: a2ui_renderer::CustomComponentDef,
    ) -> Result<(), String> {
        self.core.register_custom_component(def)
    }

    /// 注册待响应的 action_id → (surface_id, response_path) 映射
    /// surface_id 用于响应到达时精确定位写回目标（组件 id 只在 surface 内唯一）
    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        surface_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.core
            .register_pending_response(action_id, surface_id, response_path);
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

    /// 使用 egui 渲染一帧，返回用户交互生成的客户端信封
    /// 支持增量渲染：只重渲染 dirty_surfaces 中的 surface
    pub fn render_frame(
        &mut self,
        ctx: &egui::Context,
    ) -> RenderResult<Vec<a2ui_core::ClientEnvelope>> {
        let mapper = WidgetMapper;
        let mut all_actions: Vec<a2ui_core::ClientEnvelope> = Vec::new();

        // 确定要渲染的 surface 列表
        let surfaces_to_render: Vec<_> = if self.core.dirty_surfaces().is_empty() {
            self.core.surfaces().values().cloned().collect()
        } else {
            self.core.dirty_surfaces().iter().cloned().collect()
        };

        for surface_id in &surfaces_to_render {
            // 使用 build_tree 构建组件树
            let tree = match self.core.forest().build_tree(surface_id) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // 从组件树构建 flat widget map（传入 data model 用于路径解析）；
            // 先收集完 widget_map（core 只读借用结束）再加载图片（&mut self）
            let mut widget_map: HashMap<String, RenderableGuiWidget> = HashMap::new();
            Self::flatten_tree_to_widget_map(
                &tree,
                &mapper,
                &mut widget_map,
                self.core.custom_registry(),
                self.core.binding(surface_id),
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
                    let available_rect = ui.available_rect_before_wrap();
                    let available_width = available_rect.width();
                    let content_width = (available_width * 0.92)
                        .max(320.0)
                        .min(CONTENT_MAX_WIDTH)
                        .min(available_width);
                    let content_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            available_rect.center().x - content_width / 2.0,
                            available_rect.min.y,
                        ),
                        egui::vec2(content_width, available_rect.height()),
                    );

                    ui.allocate_ui_at_rect(content_rect, |ui| {
                        ui.set_min_size(content_rect.size());
                        ui.set_width(content_width);
                        mapper.render_gui_widget(
                            &root_clone,
                            ui,
                            &widget_map,
                            &mut response_tracker,
                            &mut user_events,
                            &image_textures,
                            0,
                        );
                    });
                });

                // 处理收集到的用户事件，生成客户端信封
                for event in user_events {
                    if let Ok(Some(envelope)) = pollster::block_on(self.handle_user_event(event)) {
                        all_actions.push(envelope);
                    }
                }
            }
        }

        self.core.clear_dirty();
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
        tracing::trace!(surface_id = %msg.surface_id, "createSurface");
        // egui 无 surface 级渲染缓存，CoreEffects 忽略
        self.core
            .create_surface(msg)
            .await
            .map(|(handle, _)| handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        self.core.update_components(msg).await.map(|_| ())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        tracing::debug!(surface_id = %msg.surface_id, path = ?msg.path, "updateDataModel");
        self.core.update_data_model(msg).await.map(|_| ())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        tracing::info!(surface_id = %msg.surface_id, "deleteSurface");
        self.core.delete_surface(msg).await.map(|_| ())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        self.core.action_response(msg).await.map(|_| ())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        self.core.call_function(msg).await
    }

    async fn render(&mut self) -> RenderResult<()> {
        // 实际渲染由 render_frame（需要 egui::Context）执行
        Ok(())
    }

    async fn handle_user_event(
        &mut self,
        event: UserEvent,
    ) -> RenderResult<Option<a2ui_core::ClientEnvelope>> {
        // KeyPress 是渲染器本地行为（docs/refactor-step0 D7）：
        // Enter/空格 转译为焦点组件的 Click 再交公共核心（有声明 action
        // 才发消息）；egui 的常规事件由 render_frame 内部产生，Tab 导航
        // egui 原本没有。focused_component 现无写入路径（待焦点专项）。
        let event = match event {
            UserEvent::KeyPress { key } => match key.as_str() {
                "Enter" | " " => match self.focused_component.clone() {
                    Some(comp_id) => UserEvent::Click {
                        component_id: comp_id,
                    },
                    None => return Ok(None),
                },
                _ => return Ok(None),
            },
            other => other,
        };
        let (envelope, _effects) = self.core.handle_user_event(&event).await?;
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::json;

    #[test]
    fn test_gui_renderer_new() {
        let renderer = GuiRenderer::new();
        assert!(renderer.core.surfaces().is_empty());
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
        let catalog = a2ui_core::Catalog::new("basic").with_instructions("Basic catalog");
        assert!(renderer.register_catalog(catalog).is_ok());
    }

    // --- 用户事件（docs/refactor-step0 规范语义）---

    #[tokio::test]
    async fn test_text_input_writes_back_to_data_model() {
        // 旧断言（合成 input 消息 + context dataModel 快照）→ 新断言：
        // 规范：被动输入变更不触发网络请求，只写回数据模型并标脏
        let mut renderer = GuiRenderer::new();
        let field: Component = Component::from_value(json!({
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
        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();

        let envelope = renderer
            .handle_user_event(UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap();

        assert!(envelope.is_none(), "TextInput 不应产生消息");
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/form/username"),
            Some(&json!("alice")),
            "输入值应写回 DataModel"
        );
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_check_toggle_and_slider_write_back_to_data_model() {
        // 旧断言（合成 toggle/slider_change 消息）→ 新断言：只写回 + 标脏
        let mut renderer = GuiRenderer::new();
        let checkbox: Component = Component::from_value(json!({
            "component":"CheckBox","id":"cb","checked":{"path":"/agree"}
        }))
        .unwrap();
        let slider: Component = Component::from_value(json!({
            "component":"Slider","id":"sl","value":{"path":"/volume"},"min":0,"max":100
        }))
        .unwrap();
        let root: Component = Component::from_value(json!({
            "component":"Column","id":"root","children":["cb","sl"]
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, checkbox, slider]),
                data_model: Some(json!({"agree": false, "volume": 0})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();

        let toggle_envelope = renderer
            .handle_user_event(UserEvent::CheckToggle {
                component_id: ComponentId::new("cb").unwrap(),
                checked: true,
            })
            .await
            .unwrap();
        let slider_envelope = renderer
            .handle_user_event(UserEvent::SliderChange {
                component_id: ComponentId::new("sl").unwrap(),
                value: 42.5,
            })
            .await
            .unwrap();

        assert!(toggle_envelope.is_none(), "CheckToggle 不应产生消息");
        assert!(slider_envelope.is_none(), "SliderChange 不应产生消息");
        let binding = renderer.core.binding("s1").unwrap();
        assert_eq!(binding.get("/agree"), Some(&json!(true)));
        assert_eq!(binding.get("/volume"), Some(&json!(42.5)));
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_click_without_declared_action_emits_nothing() {
        let mut renderer = GuiRenderer::new();
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
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none(), "无声明 action 的组件交互不发送消息");
    }

    #[tokio::test]
    async fn test_click_with_declared_action_emits_spec_envelope() {
        let mut renderer = GuiRenderer::new();
        let btn: Component = Component::from_value(json!({
            "id":"btn","component":"Button","child":"lbl",
            "action":{"event":{"name":"submit"}}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![btn]),
                data_model: None,
            })
            .await
            .unwrap();

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap()
            .expect("声明式 action 应产生消息");
        let value = envelope.to_value().unwrap();
        assert_eq!(value["action"]["name"], "submit");
        assert_eq!(value["action"]["surfaceId"], "s1");
        assert_eq!(value["action"]["sourceComponentId"], "btn");
    }

    #[tokio::test]
    async fn test_enter_without_focus_emits_nothing() {
        // egui 的 focused_component 现无写入路径（待焦点专项）：
        // Enter/空格 无焦点时不产生消息
        let mut renderer = GuiRenderer::new();
        let envelope = renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none());
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
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();
        assert!(renderer.core.dirty_surfaces().is_empty());

        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Bob")),
            })
            .await
            .unwrap();

        assert!(renderer.core.dirty_surfaces().contains("s1"));
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
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();

        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/other/path".into()),
                value: Some(json!("value")),
            })
            .await
            .unwrap();

        assert!(renderer.core.dirty_surfaces().is_empty());
    }

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = GuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        // 注册成功，不会 panic
        assert!(renderer.core.custom_registry().is_registered("MyChart"));
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
            Component::from_json(r#"{"id":"u1","component":"UnknownType"}"#).unwrap();
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
            Component::from_json(r#"{"id":"c1","component":"MyChart"}"#).unwrap();
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
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"result": "pending"})),
            })
            .await
            .unwrap();
        // createSurface 现在会标脏（核心语义），断言前先清空
        renderer.core.clear_dirty();

        renderer.register_pending_response("action-1", "s1", "/result");

        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await
            .unwrap();

        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }
}
