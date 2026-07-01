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
use iced::widget::image;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::Duration;

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
pub struct IcedRenderer {
    /// Surface 句柄 → SurfaceId 映射
    pub surfaces: HashMap<SurfaceHandle, String>,
    /// 组件森林（所有 Surface 的组件树）
    pub forest: ComponentForest,
    /// DataModel 绑定（surface_id → DataBinding）
    pub data_bindings: HashMap<String, DataBinding>,
    /// 依赖图
    pub dependency_graph: DependencyGraph,
    /// 函数调度器
    pub dispatcher: FunctionDispatcher,
    /// Catalog 注册表
    catalog_registry: CatalogRegistry,
    /// action_id → response_path 映射
    pub pending_responses: HashMap<String, String>,
    /// Surface 的 sendDataModel 标记
    pub send_data_model: HashMap<String, bool>,
    /// 需要增量重渲染的 surface 集合
    pub dirty_surfaces: HashSet<String>,
    /// Surface LRU 驱逐管理器
    surface_lru: SurfaceLru,
    /// 自定义组件注册表
    pub custom_registry: CustomComponentRegistry,
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
    /// Surface ID 列表（保持顺序）
    pub surface_order: Vec<String>,
}

const MAX_SURFACES: usize = 100;
const MAX_COMPONENTS_PER_SURFACE: usize = 1000;

impl IcedRenderer {
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            forest: ComponentForest::new(),
            data_bindings: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            dispatcher: FunctionDispatcher::new(),
            catalog_registry: CatalogRegistry::with_defaults(),
            pending_responses: HashMap::new(),
            send_data_model: HashMap::new(),
            dirty_surfaces: HashSet::new(),
            surface_lru: SurfaceLru::new(MAX_SURFACES, Some(Duration::from_secs(600))),
            custom_registry: CustomComponentRegistry::new(),
            text_input_values: RefCell::new(HashMap::new()),
            checkbox_values: RefCell::new(HashMap::new()),
            slider_values: RefCell::new(HashMap::new()),
            image_cache: RefCell::new(HashMap::new()),
            image_handle_cache: RefCell::new(HashMap::new()),
            tree_cache: RefCell::new(HashMap::new()),
            dynamic_string_cache: RefCell::new(HashMap::new()),
            profile: RefCell::new(IcedRenderProfile::default()),
            surface_order: Vec::new(),
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

        let tree = self.forest.build_tree(surface_id)?;
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

    fn register_component_dependencies(&mut self, comp: &Component) {
        self.dependency_graph.remove_component(comp.id());
        for path in extract_paths(comp) {
            self.dependency_graph
                .register_dependency(comp.id().clone(), path);
        }
    }

    fn register_expanded_dependencies(
        &mut self,
        surface_id: &str,
        component_ids: Vec<ComponentId>,
    ) {
        for component_id in component_ids {
            if let Some(comp) = self.forest.get(surface_id, &component_id).cloned() {
                self.register_component_dependencies(&comp);
            }
        }
    }

    pub fn register_function(
        &mut self,
        name: impl Into<String>,
        callable_from: a2ui_renderer::CallableFrom,
    ) {
        self.dispatcher.register(name, callable_from);
    }

    pub fn registered_functions(&self) -> Vec<&String> {
        self.dispatcher.registered_names()
    }

    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.catalog_registry.register(catalog)
    }

    pub fn catalog_registry(&self) -> &CatalogRegistry {
        &self.catalog_registry
    }

    pub fn register_custom_component(
        &mut self,
        def: a2ui_renderer::CustomComponentDef,
    ) -> Result<(), String> {
        self.custom_registry.register(def)
    }

    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.pending_responses
            .insert(action_id.into(), response_path.into());
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

#[async_trait::async_trait]
impl Renderer for IcedRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        if let Some(victim_id) = self.surface_lru.find_victim(self.surfaces.len()) {
            self.forest.remove_surface(&victim_id).ok();
            self.data_bindings.remove(&victim_id);
            self.surfaces.retain(|_, sid| sid != &victim_id);
            self.dirty_surfaces.remove(&victim_id);
            self.send_data_model.remove(&victim_id);
            self.surface_lru.remove(&victim_id);
            self.invalidate_surface_render_cache(&victim_id);
            self.surface_order.retain(|s| s != &victim_id);
        }

        if self.surfaces.len() >= MAX_SURFACES {
            return Err(a2ui_renderer::error::RendererError::SurfaceLimitExceeded {
                current: self.surfaces.len(),
                max: MAX_SURFACES,
            });
        }

        if !self.catalog_registry.registered_ids().is_empty() {
            if !self.catalog_registry.has_catalog(&msg.catalog_id) {
                return Err(a2ui_renderer::error::RendererError::CatalogNotFound(
                    msg.catalog_id.clone(),
                ));
            }
        }

        let handle = SurfaceHandle::new();
        let surface_id = msg.surface_id.clone();

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
            for comp in components {
                self.register_component_dependencies(&comp);
            }
        }

        let data_model = msg.data_model.unwrap_or(Value::Object(Default::default()));
        self.data_bindings.insert(
            surface_id.clone(),
            DataBinding::new(DataModel::new(data_model.clone())),
        );

        self.send_data_model
            .insert(surface_id.clone(), msg.send_data_model);

        if let Some(binding) = self.data_bindings.get(&surface_id) {
            let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));
            let expanded_ids =
                self.forest
                    .expand_templates(&surface_id, binding, &resolver, &self.dispatcher)?;
            self.register_expanded_dependencies(&surface_id, expanded_ids);
        }
        self.invalidate_surface_render_cache(&surface_id);

        self.surfaces.insert(handle, surface_id.clone());
        self.surface_lru.touch(&surface_id);
        self.surface_order.push(surface_id);

        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        for comp in msg.components {
            self.forest.upsert(&surface_id, comp.clone())?;
            self.register_component_dependencies(&comp);
        }
        self.invalidate_surface_render_cache(&surface_id);
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.surface_lru.touch(&surface_id);
        if let Some(binding) = self.data_bindings.get_mut(&surface_id) {
            if let Some(path) = &msg.path {
                binding.set(path, msg.value.unwrap_or(Value::Null))?;
                let affected = self.dependency_graph.on_data_change(path);
                if !affected.is_empty() {
                    for component_id in &affected {
                        self.invalidate_component_dynamic_cache(&surface_id, component_id);
                    }
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
        self.surface_lru.remove(&surface_id);
        self.dirty_surfaces.remove(&surface_id);
        self.send_data_model.remove(&surface_id);
        self.invalidate_surface_render_cache(&surface_id);
        self.surface_order.retain(|s| s != &surface_id);
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

            let mut pending_invalidations = None;
            for (surface_id, binding) in self.data_bindings.iter_mut() {
                if binding.as_value().pointer(&response_path).is_some() || response_path == "/" {
                    self.surface_lru.touch(surface_id);
                    binding.set(&response_path, write_value)?;
                    let affected = self.dependency_graph.on_data_change(&response_path);
                    if !affected.is_empty() {
                        self.dirty_surfaces.insert(surface_id.clone());
                        pending_invalidations = Some((surface_id.clone(), affected));
                    }
                    break;
                }
            }
            if let Some((surface_id, affected)) = pending_invalidations {
                for component_id in &affected {
                    self.invalidate_component_dynamic_cache(&surface_id, component_id);
                }
            }
        }
        Ok(())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        let function_name = msg.call.call.clone();
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
        Ok(())
    }

    async fn handle_user_event(
        &mut self,
        event: UserEvent,
    ) -> RenderResult<Option<a2ui_core::message::client_to_server::ActionMessage>> {
        match event {
            UserEvent::Click { component_id } => {
                for (surface_id, binding) in &self.send_data_model {
                    if *binding {
                        if let Some(data_binding) = self.data_bindings.get(surface_id) {
                            let data = data_binding.as_value().clone();
                            let mut ctx =
                                a2ui_core::message::client_to_server::ActionContext::new();
                            ctx.insert("data_model".into(), DynamicValue::Literal(data));
                            return Ok(Some(a2ui_core::message::client_to_server::ActionMessage {
                                name: "click".into(),
                                surface_id: surface_id.clone(),
                                source_component_id: Some(component_id.as_str().to_string()),
                                context: ctx,
                                want_response: false,
                                response_path: None,
                                action_id: None,
                            }));
                        }
                    }
                }
                Ok(Some(a2ui_core::message::client_to_server::ActionMessage {
                    name: "click".into(),
                    surface_id: String::new(),
                    source_component_id: Some(component_id.as_str().to_string()),
                    context: std::collections::HashMap::new(),
                    want_response: false,
                    response_path: None,
                    action_id: None,
                }))
            }
            _ => Ok(None),
        }
    }
}

/// 从组件中提取所有数据路径（用于依赖图注册）
fn extract_paths(comp: &Component) -> Vec<String> {
    let props = comp.properties();
    let mut paths = Vec::new();
    extract_paths_from_value(props, &mut paths);
    paths
}

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

    #[test]
    fn test_iced_renderer_new() {
        let renderer = IcedRenderer::new();
        assert!(renderer.surfaces.is_empty());
        assert!(renderer.data_bindings.is_empty());
        assert!(renderer.surface_order.is_empty());
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
        assert!(renderer.dirty_surfaces.contains("s1"));
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
            .dependency_graph
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

        let paths = renderer
            .dependency_graph
            .get_dependencies(&component_id)
            .unwrap();
        assert!(!paths.contains("/old"));
        assert!(paths.contains("/new"));
        assert!(renderer.dependency_graph.on_data_change("/old").is_empty());
        assert_eq!(renderer.dependency_graph.on_data_change("/new").len(), 1);
    }
}
