# 架构返工 · 第 2 步：serde_json 隔离与类型化 props 访问器

状态：已实施（2026-07-08，迁移计划 C0-C10 全部落地；约束白名单由 scripts/check-serde-isolation.sh 守护）
日期：2026-07-07
前置：[第 1 步 · 渲染器公共核心（RendererCore）](refactor-step1-renderer-core.md)（已实施，本设计以其收敛后的接触面为起点）
关联：ARCHITECTURE.md / CLAUDE.md 约束「`a2ui-core` 是唯一依赖 `serde_json` 的 crate，下游只依赖 `a2ui-core` 的 Rust 类型，不直接处理 JSON」（当前全面失守，本设计使其以修订后的形式可执行、可维护）

## 1. 目标与非目标

**目标**

1. 在 `a2ui-core` 补齐**类型化 props 访问器**与结构化视图（action / children / tabs / style），使四个平台渲染器不再手撕 `props.get(...).and_then(as_str)`，同时保持自定义 Catalog 的开放性（任意键仍可表达）。
2. 在 `a2ui-core` 补齐信封的**字符串级编解码对称 API**（`to_json`），使 `a2ui-transport` 从 Cargo.toml 中真正删除 `serde_json` 依赖。
3. 四个平台渲染器 crate 与 `a2ui-cli` 的 Cargo.toml 真正删除 `serde_json` 依赖（测试代码经 `a2ui-core` 的 re-export 使用 `json!` / `Value`）。
4. 把「唯一依赖」约束修订为诚实、可守住的版本，并同步 ARCHITECTURE.md / CLAUDE.md。

**非目标**（后续批次或明确不做，见 §6）

- `a2ui-renderer` 完全去 Value——它是数据绑定引擎，Value 是其职责内的合法类型（结论见 §3.5）。
- 修复调研中发现的渲染器行为差异（CheckBox `checked` vs `value`、ChoicePicker 不支持 `dynamicStringList` 绑定等）——本步是**语义保持**的迁移，差异记录在案（§2.3），另开专项。
- 运行时按 Catalog schema 校验组件 props。
- web-react 客户端。

## 2. 调研数据

### 2.1 serde_json 使用点分类计数（生产代码，2026-07-07 逐文件核对）

| crate | pub 签名暴露 Value（A） | 内部 Value 解析（B） | 字符串/值编解码（C） | 其他（D） | 测试代码使用 |
|---|---|---|---|---|---|
| a2ui-core | **25 处签名**（catalog 6、component 3、datamodel 7、envelope 3、消息字段 6） | ~76 处 | 7 处独立 + 组件构造器内 ~32 处 `json!` | `#[from] serde_json::Error`、prelude re-export `json!` | 100+ |
| a2ui-renderer | **20 处签名**（data_binding 5、path_resolver 3、dynamic_value 5、style 1、function_dispatcher 2、custom_component 2、input_writeback 1、component_forest/renderer_core/renderer.rs 均为 **0**） | ~170 处 | 7 处（component_forest 的 Component→Value→Component 往返 4 处 + json! 3 处） | 无 | ~50 |
| a2ui-renderer-tui | 0（Value 只出现在私有函数签名） | ~32 处（widget_builder.rs） | 0 | 无 | ~82 |
| a2ui-renderer-egui | 0（同上） | ~77 处（widget_mapper.rs） | 0 | 无 | ~62 |
| a2ui-renderer-iced | 0（同上） | ~44 处（widget_mapper.rs） | 0 | 无 | ~43 |
| a2ui-renderer-web | 0（同上） | ~35 处（web_renderer.rs；html_builder.rs 为 0） | 0 | 无 | ~33 |
| a2ui-transport | 0（`Transport` trait 全强类型） | 0 | **6 处**（jsonl.rs:68/92、websocket.rs:106/171、ws_server.rs:145/187，全是 `to_string`/`from_str` 信封样板） | 无 | 0 |
| a2ui-cli | 0 | 0 | **0（src/ 零使用，依赖声明是死的）** | 无 | 1 |

**关键读数**：

- 第 1 步收敛后，`RendererCore`、`ComponentForest`、`Renderer` trait 的 pub API 已经**一处都不暴露 Value**——Value 只在 `a2ui-renderer` 的叶子模块（data_binding / path_resolver / dynamic_value / style）和四家渲染器的**私有** props 解析函数里。
- 四家渲染器生产代码合计 ~190 处 Value 解析，全部是 props 读取；没有任何编解码。这意味着**只要 props 读取被类型化访问器替代，四个平台 crate 即可整体删除依赖**。
- transport 的 6 处编解码是纯样板（`ClientEnvelope`/`ServerEnvelope` ↔ String），缺的只是 core 侧 `to_json` 方法。
- a2ui-cli 的依赖声明当前就是死代码（src/ 零使用），可立即删除。

### 2.2 信封编解码 API 现状（envelope.rs）

| 方法 | ServerEnvelope | ClientEnvelope | 消费者 |
|---|---|---|---|
| `from_json(&str) -> Result<Self>` | ✓ | ✓ | jsonl/websocket 收、ws_server 收 —— 但 transport 现在是手写 `serde_json::from_str` 而没用它 |
| `to_json(&self) -> Result<String>` | **缺** | **缺** | jsonl/websocket 发、ws_server 推 —— 三处各手写 `serde_json::to_string` |
| `to_value(&self) -> Result<Value>` | ✓ | ✓ | web 侧/测试 |
| `from_value(Value) -> Result<Self>` | 缺 | 缺 | 当前无消费者，不补（YAGNI） |

结论：补两个 `to_json` + transport 改用现有 `from_json`，6 处样板全部消失，transport 依赖可删。transport 现有错误处理是手动 `map_err` 包装 `serde_json::Error` 的 Display 字符串，改为包装 `A2uiError` 的 Display，形状不变。

### 2.3 四家 widget_mapper 读取的 props 键全集

（TUI=widget_builder.rs，egui/iced=widget_mapper.rs，web=web_renderer.rs；「动态」= 值可为 `{"path":...}` / `{"call":...}` 包装而非裸标量）

| 键 | 类型 | 动态语义 | 读取方 | 组件 |
|---|---|---|---|---|
| `text` | string | ✓（四家统一走 `resolve_dynamic_string_prop*`） | 全部 | Text / Button(iced,web 直读) |
| `label` | string | ✓ | 全部 | CheckBox / DateTimeInput / ChoicePicker(iced) / Modal(web 兜底) |
| `value` | string | ✓ | 全部 | TextField |
| `value` | bool | ✓（egui/iced/web；TUI 只读裸值） | egui/iced/web | CheckBox |
| `checked` | bool | ✗（TUI 唯一来源；egui/web 兜底；iced 不读；**非规范键**） | TUI/egui/web | CheckBox |
| `value`/`min`/`max` | number | value ✓（egui/iced/web）；min/max 规范即裸 number | 全部 | Slider |
| `placeholder` | string | ✓ | 全部 | TextField |
| `variant` | string | ✗ | TUI/egui/web(Button)、egui/iced(TextField)、web(Text) | Button / TextField / Text |
| `url` | string | ✓ | 全部 | Image / Video / AudioPlayer |
| `name` | string | ✓ | 全部 | Icon |
| `description` | string | ✓ | 仅 TUI | AudioPlayer |
| `width`/`height` | number \| "fill" \| "shrink" | ✗ | egui/iced | Image |
| `child` | ComponentId(string) | ✗（ID 引用） | 全部 | Button / Card |
| `children` | array\<string\> **或** `{template, path}` | ✗（模板展开在核心层） | 全部 + `expand_templates` | Row / Column / List |
| `content` / `trigger` | ComponentId(string) | ✗ | 全部 | Modal |
| `title` | string | ✗ | 仅 web | Modal |
| `tabs` → `tabs[].title` / `tabs[].child` | array\<object\> | ✗ | 全部（**四家各写一遍**） | Tabs |
| `options` | array\<string\> | ✗（`{"path":...}` 会被静默过滤） | TUI/egui/iced/web | ChoicePicker |
| `value` | array\<string\> | ✗（规范是 `dynamicStringList`，四家均未实现绑定） | TUI/egui/web | ChoicePicker |
| `action` → `event.name/context/wantResponse/actionId/responsePath` | object | context 值 ✓（三态全支持） | **仅 RendererCore 一处**（renderer_core.rs:490-538） | Button 等可交互组件 |
| `style` → `fontSize/strong/color/fill/padding/spacing{,.x,.y}/radius` | object | ✗ | **仅 style.rs 一处**（四家共享） | 通用 |

只被 builder 写入、无渲染器消费的键（不需要访问器，记录备查）：`posterUrl`(Video)、`steps`(Slider)、`fit`(Image)、`displayStyle`(ChoicePicker)、`enableDate`/`enableTime`/`min`/`max`(DateTimeInput)、`label`(Slider/TextField)。

**重复度**：`props.get(k).and_then(|v| v.as_*())` 模式四家合计 60+ 次；其中「同一逻辑各写一遍」的有三组——`resolve_dynamic_bool`（egui/iced/web 三份几乎逐字相同）、`resolve_dynamic_number`（同前三份）、tabs 解析（四份）、options 解析（TUI/egui/web 三份）。字符串类是良性状态（四家都调 `a2ui-renderer::dynamic_value` 的同一函数）。

### 2.4 Component API 与 Catalog 开放性约束

- `Component { component_type, common(id/accessibility/weight), #[serde(flatten)] properties: Value }`——props 是 flatten 的任意 JSON object，**无键白名单、无运行时校验**；唯一读出口是 `pub fn properties(&self) -> &Value`（component.rs:729）。
- `Catalog.components: HashMap<String, Value>` 只是描述性 schema 存储，`validate()` 不遍历校验组件实例。自定义 Catalog / `CustomComponentRegistry` 允许任意组件名 + 任意 props。
- **约束推论**：任何类型化方案必须是「视图/访问器」而非「封闭枚举」——props 的存储形态（Value）与开放性不能动。
- `DynamicValue<T = Value>` 已定义在 a2ui-core（component.rs:104，`#[serde(untagged)]`，三变体 `Path { path }` / `FunctionCall { call, args }` / `Literal(T)`）——类型化访问器的返回类型现成，不需要新类型。

## 3. 设计

### 3.1 类型化 props 访问器：形态对比与选择

| 方案 | 形态 | 开放性 | 调用侧改动 | 否决/采纳理由 |
|---|---|---|---|---|
| **A：Component 上的 `prop_*` 方法系列**（采纳） | `comp.prop_str("variant")`、`comp.prop_dynamic_str("text") -> Option<DynamicValue<String>>` | 键是任意 `&str`，天然开放 | 最小：逐处机械替换 | 无生命周期噪音、可发现性好；键拼错仍是运行期 None——用**键名常量**缓解（见下） |
| B：强类型 `enum KnownProps` / 每组件 props struct | `TextProps { text: DynamicValue<String>, ... }` 整体反序列化 | **差**：自定义 Catalog 的任意键无处安放；每加组件要改 core 枚举，直接踩「不要修改 core 消息类型而不更新所有下游」的雷 | 大 | 否决：与 Catalog 开放性正面冲突；且 `#[serde(untagged)]` 的 DynamicValue 嵌套在大 struct 里反序列化失败时错误信息不可读 |
| C：`TypedProps<'a>` 独立视图结构 | `comp.props().str_("variant")` | 同 A | 同 A + 多一跳 | 与 A 实质等价，只是把方法从 Component 挪到视图；多一层间接没有换来任何隔离收益，且 `properties()` 与 `props()` 并存易混淆。否决 |

**采纳 A + 结构化视图混合**：标量/动态值走 `prop_*` 方法（开放键）；**结构已知的规范结构**（action 声明、children、tabs）走专门的 serde 反序列化视图类型（见 §3.3）。这是「Basic Catalog 强类型 + 自定义 Catalog 开放性」的平衡点：规范结构获得编译期保障，任意自定义键退化为 `prop_*`/`properties()` 逃生门，两边都不牺牲。

### 3.2 `prop_*` 访问器 API（a2ui-core, component.rs 新增）

```rust
impl Component {
    // ---- 裸标量（不含动态语义的键：variant/title/child/content/trigger...）----
    pub fn prop_str(&self, key: &str) -> Option<&str>;
    pub fn prop_bool(&self, key: &str) -> Option<bool>;
    pub fn prop_f64(&self, key: &str) -> Option<f64>;
    pub fn prop_str_list(&self, key: &str) -> Option<Vec<&str>>;   // options / value(ChoicePicker)
    pub fn prop_component_id(&self, key: &str) -> Option<ComponentId>; // child/content/trigger，语义化别名

    // ---- 动态值（值可为 {"path":..} / {"call":..} / 字面量）----
    pub fn prop_dynamic_str(&self, key: &str) -> Option<DynamicValue<String>>;
    pub fn prop_dynamic_bool(&self, key: &str) -> Option<DynamicValue<bool>>;
    pub fn prop_dynamic_f64(&self, key: &str) -> Option<DynamicValue<f64>>;

    // ---- 结构化视图（§3.3）----
    pub fn children_decl(&self) -> Option<ChildrenDecl>;
    pub fn tabs_decl(&self) -> Option<Vec<TabDecl>>;
    pub fn action_decl(&self) -> Option<ActionDecl>;
    pub fn style_decl(&self) -> Option<StyleDecl>;

    // ---- 逃生门（保留，CLAUDE.md 禁止删除）----
    pub fn properties(&self) -> &Value;   // 原样保留；文档注明「优先用 prop_* / *_decl」
}
```

要点：

- `prop_dynamic_*` 内部用 `serde_json::from_value::<DynamicValue<T>>(v.clone())` 实现，`untagged` 顺序（Path → FunctionCall → Literal）已在 core 定义并有注释保障，行为与四家手写分支一致；类型不符返回 `None`（与现状 `.as_*()` 失败即 None 的宽容语义**逐 bit 对齐**，不新增报错路径——语义保持是本步红线）。
- **键名常量**：`pub mod prop_keys { pub const TEXT: &str = "text"; ... }` 收录 §2.3 全表，缓解方案 A 的拼写风险；渲染器引用常量而非裸字符串。
- 返回 `DynamicValue<T>` 而**不做解析**——路径求值需要 `DataBinding`（a2ui-renderer 的职责），core 只负责「从 JSON 形态到类型形态」。配套地，`a2ui-renderer` 新增以 `DynamicValue<T>` 为入参的解析函数（`resolve_str(dv, binding)` 等），egui/iced/web 三份重复的 `resolve_dynamic_bool/number` 由此收编为一份。现有 `dynamic_value.rs` 的 `&Value` 入参函数**保留**（禁删函数），文档标注 deprecated 指向新 API。

### 3.3 结构化视图类型（规范结构已知的 props）

全部定义在 a2ui-core，`#[derive(Deserialize)]`，从 `properties()` 的对应子树反序列化，解析失败返回 `None`（宽容对齐现状）：

```rust
/// action 声明（规范 §UserAction；现仅 RendererCore 手撕，改为此视图）
pub struct ActionDecl { pub event: EventDecl }
pub struct EventDecl {
    pub name: String,                                   // 必填；缺失时 action_decl() 整体为 None（对齐现状 warn+丢弃）
    pub context: Option<serde_json::Map<String, Value>>, // 值是 DynamicValue，由 renderer 逐个求值——Value 合法滞留 core 类型内
    pub want_response: Option<bool>,
    pub action_id: Option<String>,
    pub response_path: Option<String>,                  // 本地语义，不上线路（第 0 步决议）
}

/// children：数组形态或模板形态
pub enum ChildrenDecl {
    Ids(Vec<ComponentId>),
    Template { template: ComponentId, path: String },
}

pub struct TabDecl { pub title: String, pub child: ComponentId }

/// style 对象的结构提取（数值/开关/颜色字符串原样给出；
/// 颜色解析、f32 换算等渲染语义留在 a2ui-renderer::style）
pub struct StyleDecl {
    pub font_size: Option<f64>, pub strong: Option<bool>,
    pub color: Option<String>,  pub fill: Option<String>,
    pub padding: Option<f64>,   pub radius: Option<f64>,
    pub spacing: Option<SpacingDecl>,   // enum：Uniform(f64) | Xy { x, y }
}
```

边界说明：

- `EventDecl.context` 保留 `Map<String, Value>`——context 值本身就是任意 JSON 的 DynamicValue，强行类型化没有收益；它只被 `RendererCore::resolve_context_value` 消费，Value 不出 a2ui-renderer。
- `ChildrenDecl` **不**收编 TUI 的 `{"children":{"children":[...]}}` 双重嵌套历史兼容分支——该分支留在 TUI 私有代码里原样保留（迁移后 TUI 先试 `children_decl()`，None 再走旧分支），不让历史包袱进 core。
- Tabs 解析四家四份 → `tabs_decl()` 一份，是本设计消除重复的最大单点。

### 3.4 Value 在公共 API 的去留边界（诚实结论）

逐层结论：

| 层 | Value 去留 | 理由 |
|---|---|---|
| a2ui-core | **留**（25 处签名不动） | DataModel、Catalog schema、`FunctionResponse.value`、`CallFunctionPayload.args`、信封 metadata——这些字段在协议里就是任意 JSON，Value 是正确类型。core 是 Value 的法定居所。新增：`pub use serde_json::Value;`（lib.rs）+ prelude 已有的 `json!` re-export，构成下游的唯一入口 |
| a2ui-renderer | **留依赖，收窄暴露** | `DataBinding::get/set/as_value`、`PathResolver`、`FunctionHandler = Fn(Value) -> Result<Value>`、`CustomComponentDef.schema` 操作的都是 DataModel/任意 JSON，参数改掉是自欺（换个名字的 Value）。`component_forest` 的 Component→Value→改 children→Component 往返（4 处编解码）是真正的坏味道，但替代方案要求 core 提供 props 变异 API，属独立设计（§6）。**结论：约束修订为「a2ui-core + a2ui-renderer 是仅有的两个依赖 serde_json 的 crate」**——a2ui-renderer 本来就是与 core 并列的公共层，不是「下游」 |
| 四个平台渲染器 | **Cargo.toml 真删** | 生产代码 ~190 处 Value 解析全部是 props 读取，被 §3.2/§3.3 完全覆盖；私有 helper（`extract_children_ids`、`parse_tabs`、`resolve_dynamic_bool` 三胞胎等）的函数体改为委托新 API（函数本身保留，禁删）。测试的 `json!`/`Value` 走 core re-export。**可行性已核实：四家 pub 签名零 Value，无外部破坏** |
| a2ui-transport | **Cargo.toml 真删** | 见 §3.6 |
| a2ui-cli | **Cargo.toml 真删** | src/ 零使用，唯一测试使用点改 re-export，纯机械 |

诚实的代价声明：「删依赖」在 Cargo 语义上四个平台 crate + transport + cli 是**真删**；a2ui-renderer 做不到也**不应该做到**——style.rs 虽可经 `StyleDecl` 去掉 Value 遍历，但 DataBinding/dispatcher/forest 的 Value 是职责本体。原约束「唯一依赖」按字面永远守不住（renderer 层必须解析绑定路径指向的任意 JSON），修订后的约束才是可被 CI 检查（`cargo tree -i serde_json` 白名单）的真约束。

### 3.5 CoreEffects / DataBinding 等具体 API 的处置

- `CoreEffects`：无 Value 字段，不动。
- `DataBinding::set(path, value: Value)`：**不动**。写回的值来自用户输入 → 任意 JSON 进 DataModel，Value 正确。第 1 步后仅 `input_writeback`（renderer 内部）与测试调用，不再是平台 crate 的接触面。
- `dynamic_value.rs` 五个 `&Value` 入参函数、`style.rs::from_component_props(&Value)`：保留 + deprecated 注记，新增 `DynamicValue<T>` / `StyleDecl` 入参的对应新函数；platform crate 全部改调新函数后旧函数只剩 renderer 内部测试引用。

### 3.6 transport：编解码 API 补齐与依赖移除

a2ui-core 新增（TDD：先写往返对称测试）：

```rust
impl ServerEnvelope { pub fn to_json(&self) -> Result<String> { Ok(serde_json::to_string(self)?) } }
impl ClientEnvelope { pub fn to_json(&self) -> Result<String> { Ok(serde_json::to_string(self)?) } }
```

transport 三个实现文件的 6 个编解码点替换为 `envelope.to_json()` / `XxxEnvelope::from_json(&text)`，错误映射从「包装 serde_json::Error 的 Display」改为「包装 A2uiError 的 Display」（`TransportError` 形状不变，错误文案前缀不变）。之后 `crates/a2ui-transport/Cargo.toml` 删除 `serde_json` 行，`cargo build -p a2ui-transport` 必须绿。`from_value` 不补（无消费者）。

## 4. 迁移计划（每个 commit 红→绿→workspace 全绿）

| # | Commit | 内容 | 性质 | 验证 |
|---|---|---|---|---|
| C0 | core：信封 `to_json` ×2 + `pub use serde_json::Value` | §3.6 前半 + re-export | 纯机械 | 往返对称测试（`from_json(to_json(x)) == x`） |
| C1 | transport 切换到信封 API，**删 serde_json 依赖** | §3.6 后半 | 纯机械 | transport 全测试 + `cargo tree -p a2ui-transport -i serde_json` 无命中 |
| C2 | cli 删死依赖（src 零使用），测试改 re-export | — | 纯机械 | workspace 绿 |
| C3 | core：`prop_*` 系列 + `prop_keys` 常量 | §3.2 | **需设计评审**（API 命名/宽容语义逐条对照 §2.3 表定案） | 每个访问器对照四家现状行为写等价性单测（含 path/call/字面量/类型不符四象限） |
| C4 | core：`ActionDecl`/`ChildrenDecl`/`TabDecl`/`StyleDecl` 视图 | §3.3 | **需设计评审**（字段可选性、解析失败宽容边界） | 规范样例 + 畸形输入宽容性测试 |
| C5 | renderer：`DynamicValue<T>` 入参的 resolve 新函数；`RendererCore::handle_user_event` 改用 `action_decl()`；style 改用 `StyleDecl`；旧函数标 deprecated | §3.2 配套 | 半机械（action 路径有 warn/丢弃语义需对齐） | 第 1 步既有核心单测全绿（行为不变的最强证据） |
| C6 | TUI 迁移 + 删依赖 | 私有 helper 委托新 API；双重嵌套兼容分支保留 | 纯机械（兼容分支除外） | 现有测试 + tree 检查 |
| C7 | egui 迁移 + 删依赖 | 同上 | 纯机械 | 同上 |
| C8 | iced 迁移 + 删依赖 | 注意保留 DynamicString 缓存层（键从 Value 摘要改为 DynamicValue 摘要） | 半机械（缓存键） | 同上 + 缓存命中断言 |
| C9 | web 迁移 + 删依赖 | 同 C6 | 纯机械 | 同上 |
| C10 | 文档：ARCHITECTURE.md/CLAUDE.md 约束改为「core + renderer 双 crate 白名单」；可选 CI 检查脚本 | §3.4 | 纯机械 | — |

顺序依据：C0-C2 独立小胜仗先落袋（transport/cli 与 props 方案零耦合）；C3/C4 是唯二需要评审的设计点，评审通过前 C5-C9 不动工；平台迁移沿用第 1 步顺序（TUI 最简先趟通，iced 缓存交互最复杂靠后）。每个平台 commit 内「删依赖」与「改调用」同 commit——依赖删不掉即等价性未完成，是天然的完成度检查。

## 5. 风险

1. **`untagged` 反序列化的语义漂移**：`prop_dynamic_*` 用 `from_value::<DynamicValue<T>>` 替代手写 `.as_*()` + `get("path")` 分支，两者对畸形输入（如 `{"path": 3}`、`{"call":..}` 无 `args`）的宽容度可能有细微差异。对策：C3 的四象限等价性测试以**四家现状行为**为基准逐键断言，发现差异一律向现状对齐。
2. **克隆成本**：`prop_dynamic_*` 内部 `v.clone()` 后 `from_value`，热路径（每帧重建 widget 树的 egui/TUI）比现状的借用读多一次小对象克隆。量级：单组件个位数键 × 标量克隆，预判可忽略；iced 缓存层照旧兜底。C8 前跑一次现有 demo 的粗基准，异常再议（可退化为手写借用实现，API 不变）。
3. **行为差异被「顺手修掉」的诱惑**：CheckBox `checked`/`value` 四家不一致、ChoicePicker 绑定缺失等在迁移中极易被顺手统一——那会让等价性验证失去基线。红线：本步一律照抄现状（包括抄不一致），差异清单（§2.3）留给专项。
4. **禁删函数约束下的 API 双轨期**：旧 `&Value` 函数 + 新类型化函数并存，新代码可能误用旧 API。对策：deprecated 注记 + clippy 在 CI 报 warning；C10 文档明示新代码规则。
5. **core 新增 25+ 个 pub API 的文档负担**：CLAUDE.md 要求每个 pub API 带可运行示例——C3/C4 工作量估算需含 doctest（约访问器数 × 1 例），不可压缩。

## 6. 明确不做（并说明为什么）

- **`component_forest` 的 Component→Value→Component 往返消除**：需要 core 提供 props 变异 API（如 `with_replaced_children`），涉及 Component 不可变性设计，独立评审后另开一步；本步它留在 a2ui-renderer 内部，不影响平台 crate 删依赖。
- **`ServerEnvelope::from_value` / `ClientEnvelope::from_value`**：全仓库无消费者。
- **删除任何现有函数/放弃 `properties()` 逃生门**：CLAUDE.md 禁删 + 自定义 Catalog 需要它。
- **补齐无人消费的 props 键**（posterUrl/steps/displayStyle/enableDate...）：渲染器功能缺口，非隔离问题。
- **web-react（TS 客户端）的对应改造**：不在 Rust workspace 范围。
