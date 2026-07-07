# 架构返工 · 第 1 步：渲染器公共核心（RendererCore）设计

状态：待评审
日期：2026-07-07
前置：[第 0 步 · 事件 wire 格式决策](refactor-step0-event-wire-format.md)（本设计的 `handle_user_event` 按其落地）
关联：ARCHITECTURE.md「Surface 生命周期由状态机管理」约束（本设计使其真正生效）

## 1. 目标与非目标

**目标**

1. 把四个渲染器各持有一份的协议状态与消息处理逻辑（约 60% 重复）收敛到 `a2ui-renderer` 的单一 `RendererCore`，渲染器退化为「核心 + 平台 widget 映射」。
2. 接入 `a2ui_core::state::StateMachine`，使 `createSurface → Active → deleteSurface` 的顺序约束在消息入口处强制生效（当前定义了但零调用点）。
3. 落地第 0 步的事件 wire 格式。
4. 顺带修复一批「四份复制各缺一角」的已知缺陷（见 §6）。

**非目标**（后续批次）

- serde_json 隔离 / a2ui-core 类型化 props 访问器（第 2 步，本设计收敛接触面后做）。
- 模板克隆 `clone_and_resolve_subtree_inner` 不跟随 content/trigger/tabs 边（forest 内部问题）。
- web-react 双套协议实现收敛。
- `Renderer` trait 签名变更——trait 保持原样，各渲染器的 trait 实现变薄。

## 2. 现状：重复与发散矩阵

四家 `create_surface` 的 12 个步骤完全同构（LRU 驱逐 → MAX_SURFACES → catalog 校验 → MAX_COMPONENTS → upsert → 依赖注册 → DataBinding → sendDataModel 记录 → 模板展开 → 登记句柄 → touch），仅 iced 多缓存失效、web 多 `last_html` 清理。其余消息的差异即缺陷：

| 消息 | TUI | Web | egui | iced |
|---|---|---|---|---|
| update_components：组件限额检查 | ✗ | ✗ | ✗ | ✗ |
| update_components：依赖重注册 | ✗ | ✗ | ✗ | ✓ |
| update_components：模板展开 | ✗ | ✗ | ✗ | ✗ |
| update_components：标脏 | ✗ | ✓ | ✗ | ✗（走缓存失效） |
| update_data_model：`path=None` 处理 | 静默丢弃 | 静默丢弃 | 静默丢弃 | 静默丢弃 |
| delete_surface：清 dirty/send_data_model | ✓ | ✓ | ✗ | ✓ |
| delete_surface：清依赖图 | ✗ | ✗ | ✗ | ✗ |
| 状态机校验 | ✗ | ✗ | ✗ | ✗ |

（✓/✗ 依据 tui_renderer.rs:504-543、web_renderer.rs:552-589、gui_renderer.rs:410-445、iced_renderer.rs:328-368 逐行核对。）

`call_function` 四家逐字相同；`action_response`、`handle_user_event` 的输入回写在上一批次已同构。**每修一个 bug 要改四处、且实际总有一处漏掉**——egui 的 delete_surface 清理缺失、web 的 dataModel 跨 surface 泄漏都是复制退化的实例。

## 3. 设计

### 3.1 模块与结构体

新模块 `crates/a2ui-renderer/src/renderer_core.rs`，`lib.rs` re-export：

```rust
pub struct RendererCore {
    surfaces: HashMap<SurfaceHandle, String>,
    surface_order: Vec<String>,              // 提升自 iced：确定性顺序，四家共享
    states: HashMap<String, StateMachine>,   // ★ 新增：生命周期状态机
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
}
```

限额常量（`MAX_SURFACES = 100`、`MAX_COMPONENTS_PER_SURFACE = 1000`）随迁，四家现值相同。

### 3.2 效果回执 `CoreEffects`

iced 有 RefCell 渲染缓存、web 有 `last_html`，核心处理消息后它们必须失效对应缓存。**用返回值而非回调**（无借用纠缠、核心可独立单测）：

```rust
#[derive(Debug, Default, PartialEq)]
pub struct CoreEffects {
    /// 整 surface 缓存需失效（创建/组件更新/删除/LRU 驱逐）
    pub invalidated_surfaces: Vec<String>,
    /// 组件级缓存需失效（数据变更波及）
    pub invalidated_components: Vec<(String, ComponentId)>,
}
```

消费方式：iced 映射到 `invalidate_surface_render_cache` / `invalidate_component_dynamic_cache`；web 映射到 `last_html.remove`；TUI/egui 忽略（无缓存）。`dirty_surfaces` 留在核心内（四家语义一致），渲染器经 `core.dirty_surfaces()` / `core.clear_dirty()` 读取。

### 3.3 方法与规范流水线

```rust
impl RendererCore {
    pub async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<(SurfaceHandle, CoreEffects)>;
    pub async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<CoreEffects>;
    pub async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<CoreEffects>;
    pub async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<CoreEffects>;
    pub async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<CoreEffects>;
    pub async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse>;
    pub async fn handle_user_event(&mut self, event: &UserEvent)
        -> RenderResult<(Option<ActionMessage>, CoreEffects)>;
    // register_function / register_catalog / register_custom_component /
    // register_pending_response 等注册类 API 原样随迁；
    // 只读访问器：forest() / binding(surface_id) / surface_order() /
    // dirty_surfaces() / clear_dirty() / send_data_model(surface_id) ...
}
```

方法用 `async fn`（内部无 await）——遵循 CLAUDE.md「异步接口统一用 async fn」，且与 `Renderer` trait 对齐、渲染器直接 `self.core.xxx(msg).await`。

各消息的规范流水线（= 四家现状的并集 + 缺陷修复）：

**create_surface**
1. 状态机：`surface_id` 已存在且非 Deleted → `InvalidStateTransition` 拒绝（现状：静默覆盖重建）；新建 `StateMachine` 并 `create_surface()` 转 Active
2. LRU 驱逐（**循环驱逐至容量内**，现状单次 `find_victim` 在超限 >1 时不足）→ 被驱逐者走完整 delete 清理，进 `invalidated_surfaces`
3. `MAX_SURFACES` → 4. catalog 校验 → 5. `MAX_COMPONENTS_PER_SURFACE`
6. 逐组件 `forest.upsert` + 依赖注册 → 7. 建 DataBinding → 8. 记录 sendDataModel
9. `expand_templates` + 对展开产物注册依赖（采纳 iced 行为）
10. 登记 handle/`surface_order`/LRU touch，标脏，本 surface 进 `invalidated_surfaces`

**update_components**
1. 状态机：必须 Active（Pending 不可达、Deleted/不存在 → 错误；现状 `forest.upsert` 会静默隐式重建 surface——审查 H3，本设计封死）
2. LRU touch → 3. **更新后组件总数过 `MAX_COMPONENTS` 检查**（修复：现状四家全缺，限额可被 update 绕过）
4. upsert + **依赖重注册**（采纳 iced）→ 5. **`expand_templates`**（修复：四家全缺）
6. **标脏**（采纳 web）+ 本 surface 进 `invalidated_surfaces`

**update_data_model**
1. 状态机 Active 校验 → 2. LRU touch
3. `path: None` → **整模型替换**（修复：现状四家静默丢弃；实现按 RFC 6901 以空指针 `""` 为根，不再特判 `"/"`）
4. `binding.set` → `on_data_change` → 标脏 + 受影响组件进 `invalidated_components`

**delete_surface**
1. 状态机 Active→Deleted 校验（不存在 → `SurfaceNotFound`）
2. **先收集该 surface 全部组件 id，从依赖图移除**（修复：四家全漏，长期泄漏且陈旧依赖导致误标脏）
3. forest/bindings/send_data_model/dirty/surfaces/surface_order/LRU 全量清理（= TUI+iced 的并集，补齐 egui 缺的两项）
4. **清理指向该 surface 的 `pending_responses`**（修复泄漏）
5. `states.remove(surface_id)`——删除后允许同 id 重新创建（新生命周期）；迟到的 update 因 surface 不存在被拒，语义仍受保护。保留墓碑会使 id 永久不可复用且内存无界，故不保留。
6. 本 surface 进 `invalidated_surfaces`

**action_response / call_function**：上一批次已同构/逐字相同，原样迁入；action_response 的受影响组件进 `invalidated_components`。

**handle_user_event**：`write_back_user_event`（已有共享 helper）→ 受影响组件失效 + 标脏。消息构造按第 0 步规范：输入类事件（TextInput/CheckToggle/SliderChange）**只写回不发消息**（返回 `None`，规范：被动变更不触发网络请求）；Click 解析组件声明的 `action.event.*`（无声明返回 `None`），`surface_of` 反查必填 `surfaceId`（失败丢弃+warn）、context 绑定求值为原生 JSON 值、填 `sourceComponentId` 与 `timestamp`、sendDataModel 时经信封 `metadata` 附带**本 surface** 数据、`wantResponse+actionId` 时自动登记 pending（`responsePath` 不上线路）。KeyPress 不进核心（渲染器本地转译，见 §3.4）。

注：返回类型随之调整为 `RenderResult<(Option<ClientEnvelope>, CoreEffects)>`——metadata 在信封层，核心需返回完整信封而非裸 `ActionMessage`；`Renderer` trait 的 `handle_user_event` 签名是否同步调整在 C1 定稿（倾向 trait 不动、渲染器内部包装）。

### 3.4 各渲染器保留内容

| 渲染器 | 保留（平台特有） | 删除（迁入核心） |
|---|---|---|
| TUI | `focus_manager`、`render_frame`/`prepare_frame`/`draw_widget`、`WidgetBuilder`；KeyPress Tab/Up/Down 本地导航，Enter/空格由 focus_manager 解析为 `Click{focused}` 再交核心 | 11 个协议状态字段、6 个消息处理方法 |
| egui | `image_cache`/`load_image`、widget_map 构建、`render_frame`、`focused_component`（现为死代码，保留待焦点专项） | 同上 |
| Web | `html_builder`、`last_html`（消费 `CoreEffects` 失效）、`render_surface_html`/`render_all_html` | 同上 |
| iced | RefCell 三级缓存（消费 `CoreEffects`）、`text_input_values` 等受控组件本地状态、app.rs 管线 | 同上 + `surface_order`（升入核心） |

渲染器结构变为 `struct XxxRenderer { core: RendererCore, /* 平台字段 */ }`，`Renderer` trait 实现委托核心并应用 effects，各约 10 行/方法。

### 3.5 兼容性

- `Renderer` trait 不变；`SurfaceHandle` 语义不变。
- 各渲染器现有 pub 方法（`register_function`、`dependency_graph()`、`register_pending_response` 等）**保留为委托核心的转发方法**（CLAUDE.md 禁止删除函数）。
- iced 的裸 `pub` 字段（`data_bindings`、`forest`、`pending_responses` 等，仅自家测试在用）改为委托访问器方法；这是本设计唯一的 API 形态变更，测试同步迁移。

## 4. 状态机边界语义汇总

| 消息 \ 当前状态 | 不存在 | Active | Deleted（已移除，等价不存在） |
|---|---|---|---|
| createSurface | ✓ 创建（Pending→Active） | ✗ InvalidStateTransition | ✓ 视为新生命周期 |
| updateComponents / updateDataModel | ✗ SurfaceNotFound | ✓ | ✗ SurfaceNotFound |
| deleteSurface | ✗ SurfaceNotFound | ✓（→Deleted 后移除全部状态） | ✗ SurfaceNotFound |

注：`StateMachine` 的 Pending 态在核心内是瞬时的（create_surface 单个方法内完成 Pending→Active），不对外暴露中间态。

## 5. 迁移计划（每个 commit 红→绿→workspace 全绿）

| # | Commit | 内容 | 验证 |
|---|---|---|---|
| C0 | a2ui-core 规范符合性修复（第 0 步 C0 清单）：`ClientEnvelope` 加可选 `metadata`（D5）；`ActionMessage` 加必填 `timestamp`、`source_component_id` 改必填（D3）；`ActionResponse` wire 格式修正为信封层 `actionId` + `value`/`error` 包装（D6） | 消息类型 + 序列化测试逐项对照规范样例 | core 及全下游测试 |
| C1 | `renderer_core.rs`：结构体 + 全部消息流水线 + 状态机 + `CoreEffects` + `handle_user_event`（第 0 步格式：声明式 action 解析 `action.event.*`、输入事件只写回不发消息） | 核心单测覆盖 §3.3 每条流水线、§4 全部非法转换、第 0 步 §3 样例 | 仅新增，workspace 绿 |
| C2 | TUI 迁移（最简：只有 focus_manager 一个平台字段；验证 KeyPress 本地转译形状） | 现有测试 + 事件断言按第 0 步重写 | workspace 绿 |
| C3 | egui 迁移 | 同上 | workspace 绿 |
| C4 | Web 迁移（验证 `CoreEffects` → `last_html` 失效链路） | 同上 | workspace 绿 |
| C5 | iced 迁移（最难：effects → RefCell 缓存、pub 字段改访问器、app.rs 收敛） | 同上 + 缓存失效断言 | workspace 绿 |
| C6 | 清理与文档：删除四家残留死代码、demo/示例改为声明式 action（serve_demo 等原依赖合成事件推进流程）、更新 ARCHITECTURE.md/README | — | workspace 绿 + clippy 无新警告 |

顺序依据：TUI 平台字段最少先趟通形状；iced 缓存交互最复杂放最后；每步之间已迁移与未迁移渲染器并存（核心与旧代码互不干扰）。

## 6. 顺带修复的审查缺陷清单

| 审查条目 | 修复点 |
|---|---|
| H3（renderer）状态机建而未用、upsert 隐式重建 surface | §3.3 全部流水线的状态机前置校验 |
| M6/#6/#10/#24 update_components 绕过限额、不注册依赖、不展开模板、不标脏 | update_components 流水线 3-6 步 |
| #6/#12/#27 update_data_model `path=None` 静默丢弃 | 整模型替换 |
| M3（core 关联）`"/"` 指针特判与 RFC 6901 冲突 | 根替换改用空指针语义 |
| #13（egui）delete_surface 清理缺失 | delete 流水线统一 |
| L4/#14/#32 依赖图与 pending_responses 只增不减 | delete 流水线 2、4 步 |
| #8/#20 事件消息形状分裂、dataModel 跨 surface 泄漏、surfaceId 为空 | handle_user_event 按第 0 步格式 |
| L5（renderer）LRU 单次驱逐不足 | create 流水线循环驱逐 |
| web-react 信封 metadata 被 Rust 侧 deny_unknown_fields 拒收（互操作 bug） | C0 |

## 7. 风险

1. **改动面**：四个渲染器结构体全部重构，是全项目最大单批变更——靠 C1 先行的核心单测 + 每 commit workspace 全绿控制。
2. **行为收紧**：状态机使原先「宽容接受」的乱序消息（先 update 后 create、重复 create）变为报错。现有测试若依赖宽容行为需逐个审视是「测试错」还是「真实用例」——发现真实用例时回到本文档修订 §4。
3. **事件行为为破坏性变更**（消费方影响见第 0 步 §4）：合成事件全部消失，只有声明式 action 产生消息。依赖合成事件的现有测试与 demo（serve_demo 等待 action 推进、e2e 输入事件断言）在 C2–C6 逐一改造为声明式 action；各 commit 信息中显式声明。
4. **iced pub 字段改访问器**：形态变更，风险限于 crate 内测试（已核实无外部使用者）。
5. **C0 是协议类型变更**：`ActionMessage` 加必填字段、`ActionResponse` wire 重构直接影响所有解析双方；好在当前无外部生产消费者，此时是修正规范偏差的最低成本窗口。`timestamp` 生成的依赖选择（手写 vs `time` crate）在 C0 动工前确认。
