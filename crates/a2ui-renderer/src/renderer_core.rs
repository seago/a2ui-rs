//! 渲染器公共核心：四个平台渲染器共享的协议状态与消息处理流水线。
//!
//! 设计见 docs/refactor-step1-renderer-core.md。要点：
//! - Surface 生命周期由 [`StateMachine`] 在每条消息入口强制校验
//!   （createSurface → Active → deleteSurface，乱序/重复消息被拒绝）
//! - 消息处理返回 [`CoreEffects`]，有渲染缓存的平台（iced/web）据此失效
//! - `handle_user_event` 实现规范 wire 格式（docs/refactor-step0）：
//!   输入类事件只写回数据模型不发消息；Click 解析组件声明的
//!   `action.event.*`，无声明不发消息

use crate::catalog_registry::CatalogRegistry;
use crate::component_forest::ComponentForest;
use crate::custom_component::{CustomComponentDef, CustomComponentRegistry};
use crate::data_binding::DataBinding;
use crate::dependency_graph::DependencyGraph;
use crate::error::{RenderResult, RendererError};
use crate::function_dispatcher::{CallableFrom, FunctionDispatcher};
use crate::input_writeback::write_back_user_event;
use crate::path_resolver::PathResolver;
use crate::renderer::{SurfaceHandle, UserEvent};
use crate::surface_lru::SurfaceLru;
use a2ui_core::message::client_to_server::{ActionMessage, FunctionResponse};
use a2ui_core::message::envelope::ClientMetadata;
use a2ui_core::message::server_to_client::{
    ActionResponse, ActionResponsePayload, CallFunction, CreateSurface, DeleteSurface,
    UpdateComponents, UpdateDataModel,
};
use a2ui_core::prelude::*;
use a2ui_core::state::StateMachine;
use a2ui_core::ClientEnvelope;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// 最大并发 Surface 数量（DoS 防护）
pub const MAX_SURFACES: usize = 100;
/// 单 Surface 最大组件数量（DoS 防护）
pub const MAX_COMPONENTS_PER_SURFACE: usize = 1000;
/// Surface 空闲驱逐超时
const SURFACE_IDLE_TIMEOUT: Duration = Duration::from_secs(600);

/// 消息处理后需要平台渲染器执行的缓存失效动作。
///
/// 核心不持有平台渲染缓存（iced 的树/字符串缓存、web 的 HTML 缓存），
/// 以返回值告知失效范围；无缓存的平台（TUI/egui）可忽略。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CoreEffects {
    /// 整 surface 的渲染缓存需失效（创建/组件更新/删除/LRU 驱逐/整模型替换）
    pub invalidated_surfaces: Vec<String>,
    /// 组件级渲染缓存需失效（数据变更波及的组件）
    pub invalidated_components: Vec<(String, ComponentId)>,
}

impl CoreEffects {
    fn invalidate_surface(&mut self, surface_id: impl Into<String>) {
        self.invalidated_surfaces.push(surface_id.into());
    }
}

/// 渲染器公共核心。
///
/// 持有全部协议状态（组件森林、数据绑定、依赖图、生命周期状态机、
/// LRU、pending 响应等），实现六类服务端消息的规范处理流水线与
/// 用户事件到 action 消息的转换。平台渲染器组合本类型并将
/// `Renderer` trait 委托给它。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::RendererCore;
///
/// let core = RendererCore::new();
/// assert!(core.surface_order().is_empty());
/// ```
#[derive(Debug)]
pub struct RendererCore {
    surfaces: HashMap<SurfaceHandle, String>,
    surface_order: Vec<String>,
    states: HashMap<String, StateMachine>,
    forest: ComponentForest,
    data_bindings: HashMap<String, DataBinding>,
    dependency_graph: DependencyGraph,
    dispatcher: FunctionDispatcher,
    catalog_registry: CatalogRegistry,
    custom_registry: CustomComponentRegistry,
    pending_responses: HashMap<String, (String, String)>,
    send_data_model: HashMap<String, bool>,
    dirty_surfaces: HashSet<String>,
    surface_lru: SurfaceLru,
    /// wantResponse 时自动生成 actionId 的单调序号
    next_action_seq: u64,
}

impl Default for RendererCore {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererCore {
    /// 创建空核心
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            surface_order: Vec::new(),
            states: HashMap::new(),
            forest: ComponentForest::new(),
            data_bindings: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            dispatcher: FunctionDispatcher::new(),
            catalog_registry: CatalogRegistry::with_defaults(),
            custom_registry: CustomComponentRegistry::new(),
            pending_responses: HashMap::new(),
            send_data_model: HashMap::new(),
            dirty_surfaces: HashSet::new(),
            surface_lru: SurfaceLru::new(MAX_SURFACES, Some(SURFACE_IDLE_TIMEOUT)),
            next_action_seq: 0,
        }
    }

    // ---- 注册类 API ----

    /// 注册客户端函数（callableFrom enforcement 用）
    pub fn register_function(&mut self, name: impl Into<String>, callable_from: CallableFrom) {
        self.dispatcher.register(name, callable_from);
    }

    /// 已注册函数列表
    pub fn registered_functions(&self) -> Vec<&String> {
        self.dispatcher.registered_names()
    }

    /// 注册 Catalog（catalogId 信任链校验用）
    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.catalog_registry.register(catalog)
    }

    /// Catalog 注册表只读引用
    pub fn catalog_registry(&self) -> &CatalogRegistry {
        &self.catalog_registry
    }

    /// 注册自定义组件类型
    pub fn register_custom_component(&mut self, def: CustomComponentDef) -> Result<(), String> {
        self.custom_registry.register(def)
    }

    /// 自定义组件注册表只读引用
    pub fn custom_registry(&self) -> &CustomComponentRegistry {
        &self.custom_registry
    }

    /// 手动登记待响应 action（`handle_user_event` 在 wantResponse 时自动登记）
    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        surface_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.pending_responses
            .insert(action_id.into(), (surface_id.into(), response_path.into()));
    }

    // ---- 只读访问器 ----

    /// 组件森林只读引用
    pub fn forest(&self) -> &ComponentForest {
        &self.forest
    }

    /// 指定 surface 的数据绑定
    pub fn binding(&self, surface_id: &str) -> Option<&DataBinding> {
        self.data_bindings.get(surface_id)
    }

    /// 依赖图只读引用
    pub fn dependency_graph(&self) -> &DependencyGraph {
        &self.dependency_graph
    }

    /// Surface 创建顺序（确定性遍历用）
    pub fn surface_order(&self) -> &[String] {
        &self.surface_order
    }

    /// 句柄 → surface_id 映射
    pub fn surfaces(&self) -> &HashMap<SurfaceHandle, String> {
        &self.surfaces
    }

    /// 需要重渲染的 surface 集合
    pub fn dirty_surfaces(&self) -> &HashSet<String> {
        &self.dirty_surfaces
    }

    /// 清空脏标记（渲染器完成一帧后调用）
    pub fn clear_dirty(&mut self) {
        self.dirty_surfaces.clear();
    }

    /// 指定 surface 是否开启 sendDataModel
    pub fn send_data_model_enabled(&self, surface_id: &str) -> bool {
        self.send_data_model
            .get(surface_id)
            .copied()
            .unwrap_or(false)
    }

    // ---- 消息处理流水线 ----

    /// createSurface：生命周期起点。
    /// 同 id 重复创建被状态机拒绝；删除后的 id 可重新创建（新生命周期）。
    pub async fn create_surface(
        &mut self,
        msg: CreateSurface,
    ) -> RenderResult<(SurfaceHandle, CoreEffects)> {
        let mut effects = CoreEffects::default();
        let surface_id = msg.surface_id.clone();

        // 1. 状态机：id 已存在（Active）时 create 转换非法
        if let Some(sm) = self.states.get_mut(&surface_id) {
            sm.create_surface()?; // 必然 Err(InvalidStateTransition)
        }

        // 2. LRU 驱逐：以「即将插入后」的数量判定，循环驱逐直至容量内
        //    （同时排空 idle 超时的 surface；一次 find_victim 可能不够）
        while let Some(victim) = self.surface_lru.find_victim(self.data_bindings.len() + 1) {
            self.purge_surface(&victim, &mut effects);
        }

        // 3. Surface 限额
        if self.data_bindings.len() >= MAX_SURFACES {
            return Err(RendererError::SurfaceLimitExceeded {
                current: self.data_bindings.len(),
                max: MAX_SURFACES,
            });
        }

        // 4. Catalog 信任链校验（空注册表时跳过，向后兼容）
        if !self.catalog_registry.registered_ids().is_empty()
            && !self.catalog_registry.has_catalog(&msg.catalog_id)
        {
            return Err(RendererError::CatalogNotFound(msg.catalog_id.clone()));
        }

        // 5. 组件限额
        if let Some(components) = &msg.components {
            if components.len() > MAX_COMPONENTS_PER_SURFACE {
                return Err(RendererError::ComponentLimitExceeded {
                    surface_id: surface_id.clone(),
                    current: components.len(),
                    max: MAX_COMPONENTS_PER_SURFACE,
                });
            }
        }

        // 状态机转 Active（此后步骤不再失败于校验类错误）
        let mut sm = StateMachine::new(surface_id.clone());
        sm.create_surface()?;
        self.states.insert(surface_id.clone(), sm);

        // 6. 注册组件与依赖
        if let Some(components) = msg.components {
            for comp in &components {
                self.forest.upsert(&surface_id, comp.clone())?;
            }
            for comp in &components {
                self.register_component_dependencies(comp);
            }
        }

        // 7. 数据绑定
        let data_model = msg.data_model.unwrap_or(Value::Object(Default::default()));
        self.data_bindings.insert(
            surface_id.clone(),
            DataBinding::new(DataModel::new(data_model)),
        );

        // 8. sendDataModel 标记
        self.send_data_model
            .insert(surface_id.clone(), msg.send_data_model);

        // 9. 模板展开 + 对展开产物注册依赖
        self.expand_templates_and_register(&surface_id)?;

        // 10. 登记与标脏
        let handle = SurfaceHandle::new();
        self.surfaces.insert(handle, surface_id.clone());
        self.surface_order.push(surface_id.clone());
        self.surface_lru.touch(&surface_id);
        self.dirty_surfaces.insert(surface_id.clone());
        effects.invalidate_surface(surface_id);

        Ok((handle, effects))
    }

    /// updateComponents：surface 必须处于 Active（不存在/已删除均拒绝，
    /// 不再经 forest.upsert 隐式重建 surface）。
    pub async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<CoreEffects> {
        let mut effects = CoreEffects::default();
        let surface_id = msg.surface_id.clone();

        // 1. 状态机
        self.states
            .get(&surface_id)
            .ok_or_else(|| RendererError::SurfaceNotFound(surface_id.clone()))?
            .update_components()?;

        // 2. LRU touch
        self.surface_lru.touch(&surface_id);

        // 3. 组件限额（更新前预检，避免半更新状态）
        let existing = self.forest.component_count(&surface_id);
        let added = msg
            .components
            .iter()
            .filter(|c| self.forest.get(&surface_id, c.id()).is_none())
            .count();
        if existing + added > MAX_COMPONENTS_PER_SURFACE {
            return Err(RendererError::ComponentLimitExceeded {
                surface_id: surface_id.clone(),
                current: existing + added,
                max: MAX_COMPONENTS_PER_SURFACE,
            });
        }

        // 4. upsert + 依赖重注册
        for comp in &msg.components {
            self.forest.upsert(&surface_id, comp.clone())?;
        }
        for comp in &msg.components {
            self.register_component_dependencies(comp);
        }

        // 5. 模板展开
        self.expand_templates_and_register(&surface_id)?;

        // 6. 标脏 + 整 surface 失效
        self.dirty_surfaces.insert(surface_id.clone());
        effects.invalidate_surface(surface_id);
        Ok(effects)
    }

    /// updateDataModel：`path: None` 为整模型替换（原实现静默丢弃）。
    pub async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<CoreEffects> {
        let mut effects = CoreEffects::default();
        let surface_id = msg.surface_id.clone();

        self.states
            .get(&surface_id)
            .ok_or_else(|| RendererError::SurfaceNotFound(surface_id.clone()))?
            .update_data_model()?;
        self.surface_lru.touch(&surface_id);

        let binding = self
            .data_bindings
            .get_mut(&surface_id)
            .ok_or_else(|| RendererError::SurfaceNotFound(surface_id.clone()))?;

        match &msg.path {
            None => {
                // 整模型替换：所有绑定该 surface 的组件都可能受影响
                *binding.as_value_mut() = msg.value.unwrap_or(Value::Object(Default::default()));
                self.dirty_surfaces.insert(surface_id.clone());
                effects.invalidate_surface(surface_id);
            }
            Some(path) => {
                binding.set(path, msg.value.unwrap_or(Value::Null))?;
                let affected = self.dependency_graph.on_data_change(path);
                if !affected.is_empty() {
                    self.dirty_surfaces.insert(surface_id.clone());
                    for component_id in affected {
                        effects
                            .invalidated_components
                            .push((surface_id.clone(), component_id));
                    }
                }
            }
        }
        Ok(effects)
    }

    /// deleteSurface：Active → Deleted 后移除全部状态（同 id 可重新创建；
    /// 迟到的 update 因 surface 不存在被拒）。
    pub async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<CoreEffects> {
        let mut effects = CoreEffects::default();
        let surface_id = msg.surface_id.clone();

        self.states
            .get_mut(&surface_id)
            .ok_or_else(|| RendererError::SurfaceNotFound(surface_id.clone()))?
            .delete_surface()?;

        self.purge_surface(&surface_id, &mut effects);
        Ok(effects)
    }

    /// actionResponse：按登记时记录的 surface 精确写回。
    pub async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<CoreEffects> {
        let mut effects = CoreEffects::default();
        let action_id = msg.action_id.clone();
        let Some((surface_id, response_path)) = self.pending_responses.remove(&action_id) else {
            return Ok(effects);
        };

        let write_value = match &msg.response {
            ActionResponsePayload::Success(v) => v.clone(),
            ActionResponsePayload::Error(err) => Value::String(err.message.clone()),
        };

        match self.data_bindings.get_mut(&surface_id) {
            Some(binding) => {
                self.surface_lru.touch(&surface_id);
                binding.set(&response_path, write_value)?;
                let affected = self.dependency_graph.on_data_change(&response_path);
                if !affected.is_empty() {
                    self.dirty_surfaces.insert(surface_id.clone());
                    for component_id in affected {
                        effects
                            .invalidated_components
                            .push((surface_id.clone(), component_id));
                    }
                }
            }
            None => {
                tracing::warn!(
                    "action response {} targets missing surface {}, dropped",
                    action_id,
                    surface_id
                );
            }
        }
        Ok(effects)
    }

    /// callFunction：经 dispatcher 强制 callableFrom 边界。
    pub async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        let function_name = msg.call.call.clone();
        let result =
            self.dispatcher
                .dispatch(&function_name, msg.call.args, CallableFrom::ClientOnly)?;
        Ok(FunctionResponse {
            function_call_id: msg.function_call_id,
            call: function_name,
            value: result,
        })
    }

    /// 用户事件处理（规范 wire 格式）：
    /// - 输入类事件（TextInput/CheckToggle/SliderChange）只写回数据模型，
    ///   不产生消息（规范：被动变更不触发网络请求）
    /// - Click 解析组件声明的 `action.event.*` 构造完整信封；无声明的
    ///   组件交互不发送消息
    /// - KeyPress 不在此处理（渲染器把 Enter/空格转译为焦点组件的 Click）
    pub async fn handle_user_event(
        &mut self,
        event: &UserEvent,
    ) -> RenderResult<(Option<ClientEnvelope>, CoreEffects)> {
        let mut effects = CoreEffects::default();

        // 1. 输入值写回声明的绑定路径
        if let Some((surface_id, path)) =
            write_back_user_event(&self.forest, &mut self.data_bindings, event)?
        {
            self.surface_lru.touch(&surface_id);
            let affected = self.dependency_graph.on_data_change(&path);
            self.dirty_surfaces.insert(surface_id.clone());
            for component_id in affected {
                effects
                    .invalidated_components
                    .push((surface_id.clone(), component_id));
            }
        }

        // 2. 只有 Click（含渲染器转译的键盘激活）可能产生消息
        let UserEvent::Click { component_id } = event else {
            return Ok((None, effects));
        };

        let Some(surface_id) = self.forest.surface_of(component_id).map(String::from) else {
            tracing::warn!(
                "click on component {} that belongs to no surface, dropped",
                component_id
            );
            return Ok((None, effects));
        };
        let Some(component) = self.forest.get(&surface_id, component_id) else {
            return Ok((None, effects));
        };

        // 3. 规范：只有声明了 server action（action.event）的组件才发送消息
        let Some(action) = component.action_decl() else {
            // 区分「未声明 action」（静默）与「声明了 event 但缺合法 name」
            // （保留现状 warn + 丢弃）
            if component
                .properties()
                .get("action")
                .and_then(|a| a.get("event"))
                .is_some()
            {
                tracing::warn!(
                    "component {} declares action.event without name, dropped",
                    component_id
                );
            }
            return Ok((None, effects));
        };
        let event_decl = action.event;

        // 4. 对声明的 context 求值（path 绑定/函数调用 → 具体值）
        let binding = self.data_bindings.get(&surface_id);
        let mut message =
            ActionMessage::event(&event_decl.name, surface_id.clone(), component_id.as_str());
        if let Some(ctx_decl) = &event_decl.context {
            for (key, raw) in ctx_decl {
                let resolved = resolve_context_value(raw, binding, &self.dispatcher);
                message = message.with_context(key, DynamicValue::Literal(resolved));
            }
        }

        // 5. wantResponse / responsePath / actionId（actionId 属实例，
        //    声明未提供时自动生成；responsePath 是本地语义，不上线路）
        if event_decl.want_response.unwrap_or(false) {
            let action_id = match event_decl.action_id {
                Some(id) => id,
                None => {
                    self.next_action_seq += 1;
                    format!("a-{}", self.next_action_seq)
                }
            };
            message.want_response = true;
            message.action_id = Some(action_id.clone());
            if let Some(response_path) = event_decl.response_path {
                self.pending_responses
                    .insert(action_id, (surface_id.clone(), response_path));
            }
        }

        // 6. sendDataModel：经信封级 metadata 附带本 surface 的数据
        let mut envelope = ClientEnvelope::v1_0(
            a2ui_core::message::client_to_server::V1_0ClientMessage::Action(message),
        );
        if self.send_data_model_enabled(&surface_id) {
            if let Some(binding) = self.data_bindings.get(&surface_id) {
                envelope = envelope.with_metadata(ClientMetadata {
                    surface_id: surface_id.clone(),
                    data_model: Some(binding.as_value().clone()),
                });
            }
        }

        Ok((Some(envelope), effects))
    }

    // ---- 内部工具 ----

    /// 移除 surface 的全部状态（delete/LRU 驱逐共用；不做状态机校验）
    fn purge_surface(&mut self, surface_id: &str, effects: &mut CoreEffects) {
        // 依赖图清理（否则陈旧依赖长期泄漏并导致误标脏）
        let component_ids: Vec<ComponentId> = self
            .forest
            .components_of(surface_id)
            .into_iter()
            .map(|c| c.id().clone())
            .collect();
        for component_id in component_ids {
            self.dependency_graph.remove_component(&component_id);
        }

        self.forest.remove_surface(surface_id).ok();
        self.data_bindings.remove(surface_id);
        self.send_data_model.remove(surface_id);
        self.dirty_surfaces.remove(surface_id);
        self.surfaces.retain(|_, sid| sid != surface_id);
        self.surface_order.retain(|sid| sid != surface_id);
        self.surface_lru.remove(surface_id);
        self.states.remove(surface_id);
        self.pending_responses
            .retain(|_, (sid, _)| sid != surface_id);
        effects.invalidate_surface(surface_id);
    }

    fn register_component_dependencies(&mut self, comp: &Component) {
        self.dependency_graph.remove_component(comp.id());
        for path in extract_paths(comp) {
            self.dependency_graph
                .register_dependency(comp.id().clone(), path);
        }
    }

    fn expand_templates_and_register(&mut self, surface_id: &str) -> RenderResult<()> {
        // 无组件的 surface（createSurface 空建、组件待后续 updateComponents
        // 到达）没有模板可展开，且 forest 中尚无该 surface 条目——直接跳过。
        if self.forest.component_count(surface_id) == 0 {
            return Ok(());
        }
        let Some(binding) = self.data_bindings.get(surface_id) else {
            return Ok(());
        };
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));
        let expanded =
            self.forest
                .expand_templates(surface_id, binding, &resolver, &self.dispatcher)?;
        for component_id in expanded {
            if let Some(comp) = self.forest.get(surface_id, &component_id).cloned() {
                self.register_component_dependencies(&comp);
            }
        }
        Ok(())
    }
}

/// 声明的 context 值求值：字面量原样、path 绑定查数据模型、
/// 函数调用经 dispatcher（客户端边界）。求值失败降级 Null 并 warn，
/// 不让单个坏绑定阻断整条 action。
fn resolve_context_value(
    raw: &Value,
    binding: Option<&DataBinding>,
    dispatcher: &FunctionDispatcher,
) -> Value {
    let dynamic: DynamicValue = match serde_json::from_value(raw.clone()) {
        Ok(d) => d,
        Err(_) => return raw.clone(),
    };
    match dynamic {
        DynamicValue::Literal(v) => v,
        DynamicValue::Path { path } => match binding.and_then(|b| b.get(&path)) {
            Some(v) => v.clone(),
            None => {
                tracing::warn!("action context path {} not found, using null", path);
                Value::Null
            }
        },
        DynamicValue::FunctionCall { call, args } => {
            match dispatcher.dispatch(&call, args, CallableFrom::ClientOnly) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("action context call {} failed: {}, using null", call, e);
                    Value::Null
                }
            }
        }
    }
}

/// 从组件属性中提取全部 `{"path": ...}` 数据路径（依赖图注册用）。
/// 宽松判定（任意含字符串 path 键的对象）：假阳性只导致多余重渲染，
/// 漏报则导致数据更新不触发重渲染——取安全方向。
fn extract_paths(comp: &Component) -> Vec<String> {
    let mut paths = Vec::new();
    collect_paths(comp.properties(), &mut paths);
    paths
}

fn collect_paths(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(p) = map.get("path").and_then(|v| v.as_str()) {
                paths.push(p.to_string());
            }
            for v in map.values() {
                collect_paths(v, paths);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_paths(v, paths);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_msg(surface_id: &str, components: Vec<Value>) -> CreateSurface {
        CreateSurface {
            surface_id: surface_id.into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(
                components
                    .into_iter()
                    .map(|v| serde_json::from_value(v).unwrap())
                    .collect(),
            ),
            data_model: None,
        }
    }

    async fn core_with_surface(data_model: Value) -> RendererCore {
        let mut core = RendererCore::new();
        let mut msg = create_msg(
            "s1",
            vec![json!({"id":"root","component":"Text","text":{"path":"/title"}})],
        );
        msg.data_model = Some(data_model);
        core.create_surface(msg).await.unwrap();
        core
    }

    // ---- 状态机边界 ----

    #[tokio::test]
    async fn create_then_update_then_delete_happy_path() {
        let mut core = core_with_surface(json!({"title":"hi"})).await;
        core.update_components(UpdateComponents {
            surface_id: "s1".into(),
            components: vec![],
        })
        .await
        .unwrap();
        core.update_data_model(UpdateDataModel {
            surface_id: "s1".into(),
            path: Some("/title".into()),
            value: Some(json!("new")),
        })
        .await
        .unwrap();
        core.delete_surface(DeleteSurface {
            surface_id: "s1".into(),
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn create_surface_without_components_then_update() {
        // 规范：createSurface 先行、组件经后续 updateComponents 到达是标准流程
        // （"A surface must be created before any updateComponents"）
        let mut core = RendererCore::new();
        let mut msg = create_msg("s1", vec![]);
        msg.components = None;
        core.create_surface(msg)
            .await
            .expect("无组件的 createSurface 必须合法");

        core.update_components(UpdateComponents {
            surface_id: "s1".into(),
            components: vec![serde_json::from_value(
                json!({"id":"root","component":"Text","text":"hi"}),
            )
            .unwrap()],
        })
        .await
        .expect("后续 updateComponents 补组件必须成功");

        assert_eq!(core.forest().component_count("s1"), 1);
        core.forest()
            .build_tree("s1")
            .expect("补齐组件后 build_tree 应可用");
    }

    #[tokio::test]
    async fn duplicate_create_is_rejected() {
        let mut core = core_with_surface(json!({})).await;
        let err = core
            .create_surface(create_msg("s1", vec![]))
            .await
            .expect_err("重复 createSurface 必须被状态机拒绝");
        assert!(
            err.to_string().to_lowercase().contains("state transition"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn update_before_create_is_rejected() {
        let mut core = RendererCore::new();
        let err = core
            .update_components(UpdateComponents {
                surface_id: "ghost".into(),
                components: vec![serde_json::from_value(
                    json!({"id":"c","component":"Text","text":"x"}),
                )
                .unwrap()],
            })
            .await
            .expect_err("createSurface 之前的 update 必须被拒绝（不得隐式建 surface）");
        assert!(matches!(err, RendererError::SurfaceNotFound(_)));
        // forest 不得被隐式创建
        assert_eq!(core.forest().component_count("ghost"), 0);
    }

    #[tokio::test]
    async fn update_after_delete_is_rejected() {
        let mut core = core_with_surface(json!({})).await;
        core.delete_surface(DeleteSurface {
            surface_id: "s1".into(),
        })
        .await
        .unwrap();
        let err = core
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/x".into()),
                value: Some(json!(1)),
            })
            .await
            .expect_err("删除后的 update 必须被拒绝");
        assert!(matches!(err, RendererError::SurfaceNotFound(_)));
    }

    #[tokio::test]
    async fn delete_missing_surface_is_rejected() {
        let mut core = RendererCore::new();
        let err = core
            .delete_surface(DeleteSurface {
                surface_id: "ghost".into(),
            })
            .await
            .expect_err("删除不存在的 surface 必须报错");
        assert!(matches!(err, RendererError::SurfaceNotFound(_)));
    }

    #[tokio::test]
    async fn recreate_after_delete_starts_fresh_lifecycle() {
        let mut core = core_with_surface(json!({"title":"old"})).await;
        core.delete_surface(DeleteSurface {
            surface_id: "s1".into(),
        })
        .await
        .unwrap();
        // 同 id 重新创建 = 新生命周期
        core.create_surface(create_msg(
            "s1",
            vec![json!({"id":"root","component":"Text","text":"fresh"})],
        ))
        .await
        .unwrap();
        assert_eq!(core.forest().component_count("s1"), 1);
    }

    // ---- 流水线补齐的缺陷 ----

    #[tokio::test]
    async fn update_components_registers_dependencies() {
        let mut core = core_with_surface(json!({"title":"a","sub":"b"})).await;
        // 经 update 新增的组件，其路径依赖必须进依赖图
        core.update_components(UpdateComponents {
            surface_id: "s1".into(),
            components: vec![serde_json::from_value(
                json!({"id":"sub","component":"Text","text":{"path":"/sub"}}),
            )
            .unwrap()],
        })
        .await
        .unwrap();
        core.clear_dirty();

        let effects = core
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/sub".into()),
                value: Some(json!("changed")),
            })
            .await
            .unwrap();
        assert!(
            core.dirty_surfaces().contains("s1"),
            "update_components 注册的依赖应使后续数据变更标脏"
        );
        assert!(effects
            .invalidated_components
            .iter()
            .any(|(_, cid)| cid.as_str() == "sub"));
    }

    #[tokio::test]
    async fn update_components_enforces_component_limit() {
        let mut core = core_with_surface(json!({})).await;
        let components: Vec<Value> = (0..MAX_COMPONENTS_PER_SURFACE)
            .map(|i| json!({"id": format!("c{i}"), "component":"Text","text":"x"}))
            .collect();
        let err = core
            .update_components(UpdateComponents {
                surface_id: "s1".into(),
                components: components
                    .into_iter()
                    .map(|v| serde_json::from_value(v).unwrap())
                    .collect(),
            })
            .await
            .expect_err("update 不得绕过组件限额");
        assert!(matches!(err, RendererError::ComponentLimitExceeded { .. }));
    }

    #[tokio::test]
    async fn update_data_model_without_path_replaces_whole_model() {
        let mut core = core_with_surface(json!({"title":"old","extra":1})).await;
        let effects = core
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: None,
                value: Some(json!({"title":"new"})),
            })
            .await
            .unwrap();
        assert_eq!(
            core.binding("s1").unwrap().as_value(),
            &json!({"title":"new"}),
            "path=None 应整模型替换而非静默丢弃"
        );
        assert!(effects.invalidated_surfaces.contains(&"s1".to_string()));
        assert!(core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn delete_surface_cleans_dependency_graph_and_pending() {
        let mut core = core_with_surface(json!({"title":"x"})).await;
        core.register_pending_response("a1", "s1", "/result");
        core.delete_surface(DeleteSurface {
            surface_id: "s1".into(),
        })
        .await
        .unwrap();

        // 依赖图已清理：同名路径变更不再波及已删组件
        let affected = core.dependency_graph.on_data_change("/title");
        assert!(affected.is_empty(), "已删 surface 的依赖必须从图中移除");
        // pending 已清理：迟到的响应不落地
        let effects = core
            .action_response(ActionResponse {
                action_id: "a1".into(),
                response: ActionResponsePayload::Success(json!("late")),
            })
            .await
            .unwrap();
        assert_eq!(effects, CoreEffects::default());
    }

    #[tokio::test]
    async fn action_response_targets_registered_surface() {
        let mut core = RendererCore::new();
        for sid in ["s1", "s2"] {
            let mut msg = create_msg(
                sid,
                vec![json!({"id":"root","component":"Text","text":{"path":"/result"}})],
            );
            msg.data_model = Some(json!({"result":"pending"}));
            core.create_surface(msg).await.unwrap();
        }
        core.register_pending_response("a1", "s2", "/result");
        core.action_response(ActionResponse {
            action_id: "a1".into(),
            response: ActionResponsePayload::Success(json!("done")),
        })
        .await
        .unwrap();
        assert_eq!(
            core.binding("s2").unwrap().get("/result"),
            Some(&json!("done"))
        );
        assert_eq!(
            core.binding("s1").unwrap().get("/result"),
            Some(&json!("pending"))
        );
    }

    // ---- handle_user_event：规范 wire 格式 ----

    #[tokio::test]
    async fn text_input_writes_back_without_emitting_message() {
        let mut core = RendererCore::new();
        let mut msg = create_msg(
            "s1",
            vec![json!({"id":"root","component":"TextField","value":{"path":"/form/name"}})],
        );
        msg.data_model = Some(json!({"form":{"name":"old"}}));
        core.create_surface(msg).await.unwrap();
        core.clear_dirty();

        let (envelope, _effects) = core
            .handle_user_event(&UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap();

        assert!(envelope.is_none(), "规范：被动输入变更不触发网络请求");
        assert_eq!(
            core.binding("s1").unwrap().get("/form/name"),
            Some(&json!("alice"))
        );
        assert!(core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn click_without_declared_action_emits_nothing() {
        let mut core = core_with_surface(json!({})).await;
        let (envelope, _) = core
            .handle_user_event(&UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none(), "无声明 action 的组件交互不发送消息");
    }

    #[tokio::test]
    async fn click_with_declared_action_emits_spec_envelope() {
        let mut core = RendererCore::new();
        let mut msg = create_msg(
            "s1",
            vec![json!({
                "id":"btn","component":"Button","label":"go",
                "action":{"event":{
                    "name":"submit_form",
                    "context":{"itemId":"123","subscribed":{"path":"/subscribe"}}
                }}
            })],
        );
        msg.data_model = Some(json!({"subscribe": true}));
        msg.send_data_model = true;
        core.create_surface(msg).await.unwrap();

        let (envelope, _) = core
            .handle_user_event(&UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap();
        let envelope = envelope.expect("声明式 action 应产生消息");

        let value = envelope.to_value().unwrap();
        assert_eq!(value["action"]["name"], "submit_form");
        assert_eq!(value["action"]["surfaceId"], "s1");
        assert_eq!(value["action"]["sourceComponentId"], "btn");
        assert!(value["action"]["timestamp"]
            .as_str()
            .unwrap()
            .ends_with('Z'));
        // context 求值：字面量原样、path 绑定解析为具体值
        assert_eq!(value["action"]["context"]["itemId"], "123");
        assert_eq!(value["action"]["context"]["subscribed"], true);
        // sendDataModel 经信封级 metadata、只含本 surface
        assert_eq!(value["metadata"]["surfaceId"], "s1");
        assert_eq!(value["metadata"]["dataModel"]["subscribe"], true);
        // responsePath 不上线路
        assert!(value["action"].get("responsePath").is_none());
    }

    #[tokio::test]
    async fn click_want_response_auto_registers_pending() {
        let mut core = RendererCore::new();
        let mut msg = create_msg(
            "s1",
            vec![json!({
                "id":"btn","component":"Button","label":"go",
                "action":{"event":{
                    "name":"fetch","wantResponse":true,"responsePath":"/result"
                }}
            })],
        );
        msg.data_model = Some(json!({"result":"pending"}));
        core.create_surface(msg).await.unwrap();

        let (envelope, _) = core
            .handle_user_event(&UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap();
        let value = envelope.unwrap().to_value().unwrap();
        assert_eq!(value["action"]["wantResponse"], true);
        let action_id = value["action"]["actionId"]
            .as_str()
            .expect("wantResponse 时 actionId 必填（自动生成）")
            .to_string();

        // 自动登记生效：响应到达即写回 /result
        core.action_response(ActionResponse {
            action_id,
            response: ActionResponsePayload::Success(json!("done")),
        })
        .await
        .unwrap();
        assert_eq!(
            core.binding("s1").unwrap().get("/result"),
            Some(&json!("done"))
        );
    }

    #[tokio::test]
    async fn click_on_unknown_component_emits_nothing() {
        let mut core = core_with_surface(json!({})).await;
        let (envelope, _) = core
            .handle_user_event(&UserEvent::Click {
                component_id: ComponentId::new("ghost").unwrap(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none());
    }

    #[tokio::test]
    async fn keypress_is_not_handled_by_core() {
        let mut core = core_with_surface(json!({})).await;
        let (envelope, effects) = core
            .handle_user_event(&UserEvent::KeyPress { key: "Tab".into() })
            .await
            .unwrap();
        assert!(envelope.is_none());
        assert_eq!(effects, CoreEffects::default());
    }

    // ---- LRU / 限额 ----

    #[tokio::test]
    async fn surface_limit_evicts_oldest_via_lru() {
        let mut core = RendererCore::new();
        for i in 0..MAX_SURFACES {
            core.create_surface(create_msg(
                &format!("s{i}"),
                vec![json!({"id":"root","component":"Text","text":"x"})],
            ))
            .await
            .unwrap();
        }
        // 第 MAX_SURFACES+1 个：LRU 驱逐最旧的 s0
        let (_, effects) = core
            .create_surface(create_msg(
                "overflow",
                vec![json!({"id":"root","component":"Text","text":"x"})],
            ))
            .await
            .unwrap();
        assert!(
            effects.invalidated_surfaces.contains(&"s0".to_string()),
            "最旧 surface 应被驱逐并出现在 effects 中"
        );
        assert!(core.binding("s0").is_none());
        assert!(core.binding("overflow").is_some());
        // 被驱逐的 id 可重新创建（状态已随驱逐移除）
        core.create_surface(create_msg(
            "s0",
            vec![json!({"id":"root","component":"Text","text":"x"})],
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn create_surface_enforces_component_limit() {
        let mut core = RendererCore::new();
        let components: Vec<Value> = (0..=MAX_COMPONENTS_PER_SURFACE)
            .map(|i| json!({"id": format!("c{i}"), "component":"Text","text":"x"}))
            .collect();
        let err = core
            .create_surface(create_msg("s1", components))
            .await
            .expect_err("超组件限额必须被拒绝");
        assert!(matches!(err, RendererError::ComponentLimitExceeded { .. }));
        // 校验失败不留半创建状态：同 id 可再次正常创建
        core.create_surface(create_msg(
            "s1",
            vec![json!({"id":"root","component":"Text","text":"x"})],
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn call_function_dispatches_with_client_boundary() {
        let mut core = RendererCore::new();
        core.register_function("formatNumber", CallableFrom::ClientOrRemote);
        let resp = core
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "formatNumber".into(),
                    args: json!({"value": 2.75, "decimals": 2}),
                },
            })
            .await
            .unwrap();
        assert_eq!(resp.function_call_id, "fc1");
        assert_eq!(resp.call, "formatNumber");
    }
}
