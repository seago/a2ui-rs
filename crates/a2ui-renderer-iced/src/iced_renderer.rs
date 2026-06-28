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
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::Duration;

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
            surface_order: Vec::new(),
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
                let paths = extract_paths(&comp);
                for path in paths {
                    self.dependency_graph
                        .register_dependency(comp.id().clone(), path);
                }
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
            self.forest
                .expand_templates(&surface_id, binding, &resolver, &self.dispatcher)?;
        }

        self.surfaces.insert(handle, surface_id.clone());
        self.surface_lru.touch(&surface_id);
        self.surface_order.push(surface_id);

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
        self.surface_lru.remove(&surface_id);
        self.dirty_surfaces.remove(&surface_id);
        self.send_data_model.remove(&surface_id);
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
            DynamicValue::Path { path: "/user/name".into() },
        );
        let result = renderer.create_surface(CreateSurface {
            surface_id: "s1".into(), catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None, send_data_model: false,
            components: Some(vec![comp]),
            data_model: Some(serde_json::json!({"user": {"name": "Alice"}})),
        }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_data_model_changes_value() {
        let mut renderer = IcedRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Path { path: "/data".into() },
        );
        renderer.create_surface(CreateSurface {
            surface_id: "s1".into(), catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None, send_data_model: false,
            components: Some(vec![comp]),
            data_model: Some(serde_json::json!({"data": "old"})),
        }).await.unwrap();

        let result = renderer.update_data_model(UpdateDataModel {
            surface_id: "s1".into(), path: Some("/data".into()), value: Some(json!("new")),
        }).await;
        assert!(result.is_ok());
    }
}
