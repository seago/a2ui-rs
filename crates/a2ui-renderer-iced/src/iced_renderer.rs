use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{CoreEffects, RenderResult, Renderer, RendererCore, SurfaceHandle, UserEvent};
use iced::widget::image;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IcedRenderProfile {
    pub tree_cache_hits: u64,
    pub tree_cache_misses: u64,
    pub element_builds: u64,
    pub dynamic_string_cache_hits: u64,
    pub dynamic_string_cache_misses: u64,
    pub image_handle_cache_hits: u64,
    pub image_handle_cache_misses: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DynamicStringCacheKey {
    pub surface_id: String,
    pub component_id: String,
    pub prop: String,
}

/// Iced 渲染器实现
///
/// 协议状态与消息处理全部委托 [`RendererCore`]，本类型只保留平台特有部分：
/// RefCell 渲染缓存（组件树 / 动态字符串 / 图片，按核心返回的
/// [`CoreEffects`] 失效）与受控组件本地状态（text_input/checkbox/slider）。
pub struct IcedRenderer {
    /// 渲染器公共核心（协议状态 + 消息流水线；pub 供同 crate 测试访问）
    pub core: RendererCore,
    /// 文本输入框本地状态（component_id → 当前输入值）
    pub text_input_values: RefCell<HashMap<String, String>>,
    /// 复选框本地状态（component_id → 当前选中状态）
    pub checkbox_values: RefCell<HashMap<String, bool>>,
    /// 滑块本地状态（component_id → 当前数值）
    pub slider_values: RefCell<HashMap<String, f64>>,
    /// 图片字节缓存（URL → 下载的图片字节，RefCell 支持 &self 读取）
    pub image_cache: RefCell<HashMap<String, Vec<u8>>>,
    /// 图片 Handle 缓存，避免每次 view 都从字节重建 Handle
    pub image_handle_cache: RefCell<HashMap<String, image::Handle>>,
    /// 组件树缓存，避免每次 view 都从 flat map 重建树
    pub tree_cache: RefCell<HashMap<String, ComponentTreeNode>>,
    /// 动态字符串解析缓存
    pub(crate) dynamic_string_cache: RefCell<HashMap<DynamicStringCacheKey, String>>,
    /// iced view/build 热点计数
    profile: RefCell<IcedRenderProfile>,
}

/// 最大并发 Surface 数量（DoS 防护）
// 已由 RendererCore 接管（a2ui_renderer::renderer_core::MAX_SURFACES），C6 统一清理
#[allow(dead_code)]
const MAX_SURFACES: usize = 100;
/// 单 Surface 最大组件数量（DoS 防护）
// 已由 RendererCore 接管（a2ui_renderer::renderer_core::MAX_COMPONENTS_PER_SURFACE），C6 统一清理
#[allow(dead_code)]
const MAX_COMPONENTS_PER_SURFACE: usize = 1000;

impl IcedRenderer {
    pub fn new() -> Self {
        Self {
            core: RendererCore::new(),
            text_input_values: RefCell::new(HashMap::new()),
            checkbox_values: RefCell::new(HashMap::new()),
            slider_values: RefCell::new(HashMap::new()),
            image_cache: RefCell::new(HashMap::new()),
            image_handle_cache: RefCell::new(HashMap::new()),
            tree_cache: RefCell::new(HashMap::new()),
            dynamic_string_cache: RefCell::new(HashMap::new()),
            profile: RefCell::new(IcedRenderProfile::default()),
        }
    }

    pub fn profile_snapshot(&self) -> IcedRenderProfile {
        *self.profile.borrow()
    }

    pub fn reset_profile(&self) {
        *self.profile.borrow_mut() = IcedRenderProfile::default();
    }

    pub(crate) fn record_element_build(&self) {
        self.profile.borrow_mut().element_builds += 1;
    }

    pub(crate) fn record_dynamic_string_cache_hit(&self) {
        self.profile.borrow_mut().dynamic_string_cache_hits += 1;
    }

    pub(crate) fn record_dynamic_string_cache_miss(&self) {
        self.profile.borrow_mut().dynamic_string_cache_misses += 1;
    }

    pub fn cached_tree(&self, surface_id: &str) -> RenderResult<ComponentTreeNode> {
        if let Some(tree) = self.tree_cache.borrow().get(surface_id).cloned() {
            self.profile.borrow_mut().tree_cache_hits += 1;
            return Ok(tree);
        }

        let tree = self.core.forest().build_tree(surface_id)?;
        self.tree_cache
            .borrow_mut()
            .insert(surface_id.to_string(), tree.clone());
        self.profile.borrow_mut().tree_cache_misses += 1;
        Ok(tree)
    }

    pub fn invalidate_surface_render_cache(&self, surface_id: &str) {
        self.tree_cache.borrow_mut().remove(surface_id);
        self.dynamic_string_cache
            .borrow_mut()
            .retain(|key, _| key.surface_id != surface_id);
    }

    pub fn invalidate_component_dynamic_cache(&self, surface_id: &str, component_id: &ComponentId) {
        let component_id = component_id.as_str();
        self.dynamic_string_cache
            .borrow_mut()
            .retain(|key, _| !(key.surface_id == surface_id && key.component_id == component_id));
    }

    /// 消费核心返回的缓存失效回执：整 surface 失效清树缓存与该 surface 的
    /// 动态字符串缓存；组件级失效只清该组件的动态字符串缓存条目
    fn apply_effects(&self, effects: &CoreEffects) {
        for surface_id in &effects.invalidated_surfaces {
            self.invalidate_surface_render_cache(surface_id);
        }
        for (surface_id, component_id) in &effects.invalidated_components {
            self.invalidate_component_dynamic_cache(surface_id, component_id);
        }
    }

    pub fn register_function(
        &mut self,
        name: impl Into<String>,
        callable_from: a2ui_renderer::CallableFrom,
    ) {
        self.core.register_function(name, callable_from);
    }

    pub fn registered_functions(&self) -> Vec<&String> {
        self.core.registered_functions()
    }

    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.core.register_catalog(catalog)
    }

    pub fn catalog_registry(&self) -> &a2ui_renderer::CatalogRegistry {
        self.core.catalog_registry()
    }

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

    /// 下载图片并缓存字节（如未缓存）
    pub fn load_image_bytes(&self, url: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.image_cache.borrow().get(url) {
            return Some(bytes.clone());
        }
        // 下载
        let response = ureq::get(url).call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).ok()?;
        self.image_cache
            .borrow_mut()
            .insert(url.to_string(), bytes.clone());
        Some(bytes)
    }

    pub fn load_image_handle(&self, url: &str) -> Option<image::Handle> {
        if let Some(handle) = self.image_handle_cache.borrow().get(url) {
            self.profile.borrow_mut().image_handle_cache_hits += 1;
            return Some(handle.clone());
        }

        let bytes = self.load_image_bytes(url)?;
        let handle = image::Handle::from_bytes(bytes);
        self.image_handle_cache
            .borrow_mut()
            .insert(url.to_string(), handle.clone());
        self.profile.borrow_mut().image_handle_cache_misses += 1;
        Some(handle)
    }
}

impl Default for IcedRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for IcedRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        tracing::trace!(surface_id = %msg.surface_id, "createSurface");
        let (handle, effects) = self.core.create_surface(msg).await?;
        self.apply_effects(&effects);
        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let effects = self.core.update_components(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        tracing::debug!(surface_id = %msg.surface_id, path = ?msg.path, "updateDataModel");
        let effects = self.core.update_data_model(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        tracing::info!(surface_id = %msg.surface_id, "deleteSurface");
        let effects = self.core.delete_surface(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        let effects = self.core.action_response(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        self.core.call_function(msg).await
    }

    async fn render(&mut self) -> RenderResult<()> {
        Ok(())
    }

    async fn handle_user_event(
        &mut self,
        event: UserEvent,
    ) -> RenderResult<Option<a2ui_core::ClientEnvelope>> {
        // iced 无键盘焦点转译（KeyPress 由核心统一忽略）；
        // 输入类事件只写回不发消息、Click 解析声明式 action，
        // 均由公共核心处理，此处只需消费缓存失效回执
        let (envelope, effects) = self.core.handle_user_event(&event).await?;
        self.apply_effects(&effects);
        Ok(envelope)
    }
}

/// 从组件中提取所有数据路径（用于依赖图注册）
// 已由 RendererCore 接管（依赖注册随消息流水线迁入核心），C6 统一清理
#[allow(dead_code)]
fn extract_paths(comp: &Component) -> Vec<String> {
    let props = comp.properties();
    let mut paths = Vec::new();
    extract_paths_from_value(props, &mut paths);
    paths
}

// 已由 RendererCore 接管（依赖注册随消息流水线迁入核心），C6 统一清理
#[allow(dead_code)]
fn extract_paths_from_value(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(p) = map.get("path").and_then(|v| v.as_str()) {
                paths.push(p.to_string());
            }
            for v in map.values() {
                extract_paths_from_value(v, paths);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                extract_paths_from_value(v, paths);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_iced_handle_text_input_writes_back() {
        // 旧断言（合成 text_input 消息 + context dataModel 快照）→ 新断言：
        // 规范：被动输入变更不触发网络请求，只写回数据模型；
        // 写回后受影响组件的动态字符串缓存被失效 + surface 标脏
        let mut renderer = IcedRenderer::new();
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
        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();
        // 预热渲染缓存，验证写回后被正确失效
        let _ = renderer.cached_tree("s1");
        renderer.dynamic_string_cache.borrow_mut().insert(
            DynamicStringCacheKey {
                surface_id: "s1".to_string(),
                component_id: "root".to_string(),
                prop: "value".to_string(),
            },
            "old".to_string(),
        );

        let envelope = renderer
            .handle_user_event(UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap();

        // 规范：被动输入变更不触发网络请求
        assert!(envelope.is_none(), "TextInput 不应产生消息");
        // 绑定路径已更新
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/form/username"),
            Some(&json!("alice"))
        );
        // 该组件的动态字符串缓存条目已被清（effects → 组件级失效链路）
        assert!(
            renderer.dynamic_string_cache.borrow().is_empty(),
            "写回后受影响组件的动态字符串缓存应被失效"
        );
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_iced_handle_check_toggle_and_slider_write_back() {
        // 旧断言（合成 check_toggle/slider_change 消息）→ 新断言：
        // 无消息 + 写回 + 标脏
        let mut renderer = IcedRenderer::new();
        let root: Component = serde_json::from_value(json!({
            "component":"Column","id":"root","children":["cb","sl"]
        }))
        .unwrap();
        let cb: Component = serde_json::from_value(json!({
            "component":"CheckBox","id":"cb","value":{"path":"/agree"}
        }))
        .unwrap();
        let sl: Component = serde_json::from_value(json!({
            "component":"Slider","id":"sl","value":{"path":"/volume"},"min":0,"max":100
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, cb, sl]),
                data_model: Some(json!({"agree": false, "volume": 0})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();
        // 预热动态字符串缓存，验证组件级失效精确到受影响组件
        for (component_id, prop) in [("cb", "value"), ("sl", "value")] {
            renderer.dynamic_string_cache.borrow_mut().insert(
                DynamicStringCacheKey {
                    surface_id: "s1".to_string(),
                    component_id: component_id.to_string(),
                    prop: prop.to_string(),
                },
                "stale".to_string(),
            );
        }

        let toggle_envelope = renderer
            .handle_user_event(UserEvent::CheckToggle {
                component_id: ComponentId::new("cb").unwrap(),
                checked: true,
            })
            .await
            .unwrap();
        assert!(toggle_envelope.is_none(), "CheckToggle 不应产生消息");

        let slider_envelope = renderer
            .handle_user_event(UserEvent::SliderChange {
                component_id: ComponentId::new("sl").unwrap(),
                value: 42.5,
            })
            .await
            .unwrap();
        assert!(slider_envelope.is_none(), "SliderChange 不应产生消息");

        let binding = renderer.core.binding("s1").unwrap();
        assert_eq!(binding.get("/agree"), Some(&json!(true)));
        assert_eq!(binding.get("/volume"), Some(&json!(42.5)));
        assert!(
            renderer.dynamic_string_cache.borrow().is_empty(),
            "写回后受影响组件的动态字符串缓存应被失效"
        );
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_iced_click_without_declared_action_emits_nothing() {
        let mut renderer = IcedRenderer::new();
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
                data_model: None,
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
    async fn test_iced_click_with_declared_action_emits_spec_envelope() {
        let mut renderer = IcedRenderer::new();
        let btn: Component = serde_json::from_value(json!({
            "id":"btn","component":"Button","child":"lbl",
            "action":{"event":{"name":"submit"}}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![btn]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
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
        // sendDataModel 经信封级 metadata 附带本 surface 数据
        assert_eq!(value["metadata"]["surfaceId"], "s1");
        assert_eq!(value["metadata"]["dataModel"]["user"]["name"], "Alice");
    }

    #[test]
    fn test_iced_renderer_new() {
        let renderer = IcedRenderer::new();
        assert!(renderer.core.surfaces().is_empty());
        assert!(renderer.core.surface_order().is_empty());
        assert!(renderer.core.binding("s1").is_none());
    }

    #[test]
    fn test_register_function() {
        let mut renderer = IcedRenderer::new();
        renderer.register_function("test_fn", a2ui_renderer::CallableFrom::ClientOrRemote);
        assert!(renderer
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "test_fn"));
    }

    #[test]
    fn test_register_catalog() {
        let mut renderer = IcedRenderer::new();
        let catalog: a2ui_core::Catalog = serde_json::from_value(json!({
            "catalogId": "basic",
            "instructions": "test",
            "components": {},
            "functions": {}
        }))
        .unwrap();
        assert!(renderer.register_catalog(catalog).is_ok());
    }

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = IcedRenderer::new();
        let def = a2ui_renderer::CustomComponentDef::new("MyWidget");
        assert!(renderer.register_custom_component(def).is_ok());
    }

    #[tokio::test]
    async fn test_create_surface_with_path_binding() {
        let mut renderer = IcedRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_data_model_changes_value() {
        let mut renderer = IcedRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Path {
                path: "/data".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(serde_json::json!({"data": "old"})),
            })
            .await
            .unwrap();

        let result = renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/data".into()),
                value: Some(json!("new")),
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cached_tree_reuses_snapshot_until_components_change() {
        let mut renderer = IcedRenderer::new();
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("old".to_string()),
        );

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root]),
                data_model: None,
            })
            .await
            .unwrap();

        let first = renderer.cached_tree("s1").unwrap();
        let second = renderer.cached_tree("s1").unwrap();
        assert_eq!(first.component.properties(), second.component.properties());

        let profile = renderer.profile_snapshot();
        assert_eq!(profile.tree_cache_misses, 1);
        assert_eq!(profile.tree_cache_hits, 1);

        let updated = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("new".to_string()),
        );
        renderer
            .update_components(UpdateComponents {
                surface_id: "s1".into(),
                components: vec![updated],
            })
            .await
            .unwrap();

        let third = renderer.cached_tree("s1").unwrap();
        assert_eq!(
            third.component.properties().get("text"),
            Some(&serde_json::json!("new"))
        );

        let profile = renderer.profile_snapshot();
        assert_eq!(profile.tree_cache_misses, 2);
        assert_eq!(profile.tree_cache_hits, 1);
    }

    #[tokio::test]
    async fn test_data_model_update_invalidates_only_affected_dynamic_cache() {
        let mut renderer = IcedRenderer::new();
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Path {
                path: "/title".into(),
            },
        );

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root]),
                data_model: Some(serde_json::json!({"title": "old"})),
            })
            .await
            .unwrap();
        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();

        renderer.dynamic_string_cache.borrow_mut().insert(
            DynamicStringCacheKey {
                surface_id: "s1".to_string(),
                component_id: "root".to_string(),
                prop: "text".to_string(),
            },
            "old".to_string(),
        );

        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/title".into()),
                value: Some(serde_json::json!("new")),
            })
            .await
            .unwrap();

        assert!(renderer.dynamic_string_cache.borrow().is_empty());
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_update_components_refreshes_dynamic_dependencies() {
        let mut renderer = IcedRenderer::new();
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Path {
                path: "/old".into(),
            },
        );

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root]),
                data_model: Some(serde_json::json!({"old": "before", "new": "after"})),
            })
            .await
            .unwrap();

        let component_id = ComponentId::new("root").unwrap();
        assert!(renderer
            .core
            .dependency_graph()
            .get_dependencies(&component_id)
            .is_some_and(|paths| paths.contains("/old")));

        let updated = Component::text(
            component_id.clone(),
            DynamicValue::Path {
                path: "/new".into(),
            },
        );
        renderer
            .update_components(UpdateComponents {
                surface_id: "s1".into(),
                components: vec![updated],
            })
            .await
            .unwrap();

        let graph = renderer.core.dependency_graph();
        let paths = graph.get_dependencies(&component_id).unwrap();
        assert!(!paths.contains("/old"));
        assert!(paths.contains("/new"));
        assert!(graph.dependents("/old").is_empty());
        assert_eq!(graph.dependents("/new").len(), 1);
    }
}
