# 架构返工 · 第 3 步：渲染器行为差异统一（输入组件规范符合性）

状态：**已实施**（2026-07-08，commits `7ca3b87` → D9 收尾；实施与设计的偏离见文末「实施记录」）
前置：第 2 步 serde_json 隔离已完成（`ef0a30a`），本步全部建立在 `prop_*` 访问器 / 结构化视图 / `resolve_*` 新 API 之上。
规范依据：[a2ui.org v1.0 规范](https://a2ui.org/specification/v1.0-a2ui/)、`catalogs/basic/catalog.json`、`common_types.json`（2026-07-08 curl 抓取核对，关键摘录见 §2.1）。

---

## 1. 目标与非目标

**目标**

1. **规范符合性修复（P0）**：规范合法的消息在四个渲染器上产出正确结果——现状下 CheckBox 与 ChoicePicker 都做不到（见 §2）。
2. **行为统一（P1）**：同一份服务端消息，四家对「组件声明状态」的解读一致；平台间只允许**渲染能力**差异（终端画不出圆角），不允许**语义解读**差异（同一消息 TUI 勾选、egui 不勾选）。
3. **补齐 ChoicePicker 交互闭环（P0）**：规范把 ChoicePicker 列为五个双向绑定输入组件之一，现状四家全部只读展示、事件模型层面就没有选择事件——补 `UserEvent` 变体、公共写回、平台交互。
4. 差异的**统一裁决落在公共层**（a2ui-core 视图 / a2ui-renderer helper），平台只做渲染映射——杜绝「四家各写一遍、越写越歪」的根因。

**非目标**

- 补齐无人消费的功能键（Image `fit`/`variant`、Video `posterUrl`、ChoicePicker `displayStyle`/`filterable`、DateTimeInput `min`/`max` 校验、Slider `steps` 吸附）——渲染器功能缺口，不是「差异」，各平台按需另立任务。
- `checks` 校验体系（`Checkable`）的实现——独立专项。
- 平台专属 UI 状态机制的统一（iced 的 `checkbox_values` 本地缓存、egui 的即时模式重读）——那是各框架受控组件模式的合理差异，见 §3.6。
- web-react（TS 客户端）——不在 Rust workspace 范围。

---

## 2. 调研数据（2026-07-08，HEAD = `ef0a30a`）

### 2.1 规范权威定义（basic catalog schema 摘录）

**CheckBox**（`required: ["component", "label", "value"]`，`unevaluatedProperties: false`）：

| 键 | 类型 | 说明 |
|---|---|---|
| `label` | DynamicString | 必填 |
| `value` | **DynamicBoolean** | 必填，勾选状态，可绑定 |

> **`checked` 不是规范键**。schema 声明 `unevaluatedProperties: false`——规范合法消息**不可能**含 `checked`。

**ChoicePicker**（`required: ["component", "options", "value"]`）：

| 键 | 类型 | 说明 |
|---|---|---|
| `options` | **array\<object\>**，项为 `{label: DynamicString, value: string}`（两者必填，`additionalProperties: false`） | 可选项列表 |
| `value` | **DynamicStringList**（字面量 string 数组 / `{path}` / `{call}`） | 当前选中值列表，规范原文「should be bound to a string array in the data model」 |
| `variant` | `"multipleSelection"` \| `"mutuallyExclusive"`，默认 `mutuallyExclusive` | 单选/多选行为 |
| `label` / `displayStyle`(`checkbox`\|`chips`) / `filterable` | — | 可选（本步不实现，见非目标） |

规范页示例（联系表单）即为对象形态：
`"options":[{"label":"Email","value":"email"},...], "value":{"path":"/contact/preference"}`。

**双向绑定契约**（规范「Two-way binding & input components」节）：TextField、CheckBox、Slider、**ChoicePicker**、DateTimeInput 五个输入组件必须——Read：渲染时从绑定 path 读值，`updateDataModel` 后重渲染；Write：用户交互时**立即**写回本地 Data Model。

**其它组件的 variant 枚举**（P1 对齐用）：
- Button：`"default" | "primary" | "borderless"`（默认 default）
- Text：`"caption" | "body"`（默认 body）
- TextField：`"longText" | "number" | "shortText" | "obscured"`（默认 shortText）
- Image：**没有 `width`/`height` 键**（尺寸走 `variant`: icon/avatar/…feature/header）；Modal：**没有 `title` 键**；AudioPlayer：**有** `description`（DynamicString，可选）。

### 2.2 四家现状矩阵（与规范对照）

**CheckBox 勾选状态读取**：

| 渲染器 | 读取逻辑 | 动态绑定 | 规范符合性 |
|---|---|---|---|
| TUI | 仅 `checked`（widget_builder.rs:434 `prop_bool(CHECKED)`） | **不支持**（`{"path":...}` 落回 false） | ✗ 规范消息（`value` 绑定）恒渲染未勾选 |
| egui | `value` → 回退 `checked`（widget_mapper.rs:221-223） | 支持 | ✓（回退键多余但无害） |
| iced | 仅 `value`（本地缓存优先，widget_mapper.rs:266-279） | 支持 | ✓ |
| web | `value` → 回退 `checked`（web_renderer.rs:231-233） | 支持 | ✓ |

写回侧已统一：`input_writeback.rs:126-135` 候选键 `["value","checked"]`，四家共享（第 1 步成果）——**读四家各异、写一家统一**，TUI 出现「写回写进 `value`、下一帧读 `checked` 读不到」的自我矛盾风险。

**ChoicePicker**：

| 维度 | 现状（四家一致地错） | 规范 |
|---|---|---|
| `options` 读取 | 四家统一走 `prop_str_list`（component.rs:852-855）——只认**裸字符串数组**，非字符串项被 `filter_map` 静默过滤 | `{label, value}` 对象数组 |
| **后果** | **规范合法消息的 options 全部被过滤 → 四家都渲染空选项列表**，无占位、无告警 | — |
| `value`（选中集）读取 | TUI/egui/web `prop_str_list(VALUE)` 只认裸数组；`{"path":...}` 得空；iced 干脆不读 | DynamicStringList，可绑定 |
| 交互写回 | **完全不存在**：`UserEvent`（renderer.rs:34-54，5 个变体）无选择事件；四家渲染纯静态（TUI 文本标记 / egui `ui.label` / iced `text()` / web 静态 `<select>`） | 双向绑定输入组件，交互立即写回 |

**其它差异（P1/P2）**：

| 键 | 现状 | 规范裁决 |
|---|---|---|
| Button `variant` | TUI/egui 只认 `primary` 二态；web 三态全支持；**iced 完全不读** | 三态枚举——iced 至少补 `primary` |
| TextField `variant` | egui/iced 只认 `obscured`；TUI/web 不读 | 四值枚举——`obscured` 是安全语义（密码明文显示），TUI/web 必须补 |
| Text `variant` | 仅 web 认 `caption` | 二值 hint——其余家按平台能力补，允许降级 |
| Image `width`/`height` | 仅 egui/iced 读 | **非规范键**（扩展），冻结现状并记录 |
| Modal `title` | 仅 web 读（`title`→`label` 兜底） | **非规范键**，规范输入下永不触发，冻结现状并记录 |
| AudioPlayer `description` | 仅 TUI 读 | **规范键**——TUI 是对的；egui/web 补齐属功能增强（P2 可选） |

### 2.3 会被本步改变行为的现有测试

| 测试 | 文件 | 现锁定行为 | 处置 |
|---|---|---|---|
| `test_checkbox_component_maps_to_checkbox_widget` | tui widget_builder.rs:992 | 只读 `checked` 字面量 | 扩展：补 `value` 优先 + 绑定用例 |
| `test_check_toggle_writes_back_without_message` | tui tui_renderer.rs:1301 | 组件只声明 `checked` 绑定时写回成功 | 保留（回退键路径仍然有效） |
| `test_checkbox_state_uses_component_id` | iced widget_mapper.rs:1094 | 本地缓存优先于声明 `value` | 保留（平台受控组件语义，§3.6） |
| `prop_str_list_filters_non_string_entries_silently` | core component.rs:1918 | 非字符串项静默过滤 | 保留（旧 API 行为不动，新视图另立） |
| ChoicePicker 四家现有测试 | 见调查报告 | 全部只测裸字符串 options 的读取渲染 | 保留并**追加**对象形态/绑定/交互用例（裸字符串兼容保留，§3.3） |

---

## 3. 设计

### 3.1 总原则：裁决进公共层，平台只留渲染

每一处差异的修复形态都是「core 出视图 / renderer 出 helper，四家改为调用」。理由与第 1、2 步相同：四家各写一遍是这些差异的产生机制，只修表象（把四份代码改一致）会在下一个功能迭代中重新漂移。

### 3.2 CheckBox：统一勾选状态解析（a2ui-renderer 新 helper）

```rust
/// 解析 CheckBox 的勾选状态：规范键 `value`（DynamicBoolean）优先，
/// 兼容键 `checked` 回退，均支持动态绑定；都缺失时 false。
pub fn checkbox_checked(component: &Component, binding: Option<&DataBinding>) -> bool
```

- 实现 = `prop_dynamic_bool(VALUE)` → `resolve_bool` → 失败则 `prop_dynamic_bool(CHECKED)` → `resolve_bool` → `false`。与 egui/web 现行逻辑一致（它们是四家中的正确实现）。
- **`checked` 回退键保留**的理由：(a) 写回候选键 `["value","checked"]` 已发布，删回退会破坏「只声明 `checked` 绑定」的既有用户（TUI 现有测试即此形态）；(b) 读宽写严是本仓库一贯的宽容语义。它是**扩展**而非规范，随本步在 ARCHITECTURE.md 记录（§3.7）。
- 四家改为调用此 helper：TUI 从「仅 `checked` 裸值」升级（**行为变化**：获得 `value` 键 + 动态绑定支持）；iced 获得 `checked` 回退（本地缓存优先逻辑保留在 helper 之外，见 §3.6）；egui/web 是纯等价重构。

### 3.3 ChoicePicker options：core 新增 `OptionDecl` 视图

```rust
/// ChoicePicker 的单个选项声明（规范 basic catalog：{label, value}）。
pub struct OptionDecl {
    pub label: DynamicValue<String>,  // 规范为 DynamicString，label 本身可绑定
    pub value: String,
}

impl Component {
    /// 解析 `options` 键为选项列表。宽容语义（逐项）：
    /// - 对象形态 {label, value}：规范主路径；label 缺失时以 value 充当。
    /// - 裸字符串 "A"：兼容旧形态，等价于 {label: "A", value: "A"}。
    /// - 其余畸形项：整项跳过（与既有视图的宽容惯例一致）。
    /// options 缺失或非数组 → None。
    pub fn options_decl(&self) -> Option<Vec<OptionDecl>>
}
```

- **裸字符串兼容形态保留**：现有四家测试、（潜在的）既有服务端都用裸字符串；`label == value` 的退化语义无歧义。这同样是扩展，记录在案。
- 与 `prop_str_list` 的关系：旧 API 原样保留（CLAUDE.md 禁删 + 其它键仍在用），ChoicePicker 调用点全部换到 `options_decl()`。
- 手写宽容提取而非 `#[derive(Deserialize)]`，理由与第 2 步偏离点 1 相同（整体反序列化会被单个畸形项拖垮整个列表）。

### 3.4 ChoicePicker value：DynamicStringList 支持

core 侧补最后一块 Dynamic 积木（规范 common_types 四个 Dynamic* 中唯一未实现的）：

```rust
// a2ui-core component.rs（复用私有 prop_dynamic<T> 泛型实现，纯增量）
pub fn prop_dynamic_str_list(&self, key: &str) -> Option<DynamicValue<Vec<String>>>

// a2ui-renderer dynamic_value.rs（模式复制 resolve_bool/resolve_f64）
pub fn resolve_str_list(dv: &DynamicValue<Vec<String>>, binding: Option<&DataBinding>) -> Option<Vec<String>>
```

`resolve_str_list` 语义与 `resolve_bool` 三胞胎对齐：Literal 直取；Path 经 `binding.get` 取值，是字符串数组则返回（数组内非字符串项按 `prop_str_list` 惯例逐项过滤），否则 None；FunctionCall → None（现状四家均不支持函数求值，语义保持）。

选中集读取统一为：`prop_dynamic_str_list(VALUE)` → `resolve_str_list` → 缺省空集。**iced 从「不读选中态」升级为渲染选中态**（行为变化，向规范收敛）。

### 3.5 ChoicePicker 交互：新事件 + 公共写回 + variant 语义

**事件**（a2ui-renderer renderer.rs，纯增量变体——`UserEvent` 是 `#[non_exhaustive]` 待确认，若不是则此为语义上的新增、下游 match 需补臂）：

```rust
UserEvent::ChoiceSelect {
    component_id: ComponentId,
    /// 交互后的完整选中值集合（不是增量 toggle）。
    values: Vec<String>,
}
```

**决策：事件携带完整新选中集**，而非「被点击项 + 由写回层做 toggle」。理由：(a) 写回层保持纯写语义，不需要读旧值再计算（`input_writeback` 现有三个变体都是「事件值 → 直写」）；(b) toggle 逻辑依赖 `variant`（单选=整体替换 / 多选=集合增删），归属交互发生地。为避免四家各写一遍 toggle，公共层出纯函数：

```rust
/// 计算点击某选项后的新选中集。mutuallyExclusive：整体替换为 [clicked]；
/// multipleSelection：clicked 在集合中则移除、否则追加（保持既有顺序）。
/// variant 缺失/非法按规范默认 mutuallyExclusive。
pub fn toggle_choice(current: &[String], clicked: &str, variant: Option<&str>) -> Vec<String>
```

**写回**（input_writeback.rs 纯增量分支）：

```rust
UserEvent::ChoiceSelect { component_id, values } => write_back_input(
    forest, bindings, component_id, &["value"], Value::Array(values → Value::String),
)
```

候选键只有 `["value"]`（规范唯一绑定键，ChoicePicker 没有历史兼容键）。`RendererCore::handle_user_event` 的统一写回入口（renderer_core.rs:462-463）自动覆盖新变体，无平台侧写回代码。

**平台交互实现**（各平台能力内，允许交互形式差异、不允许语义差异）：

| 平台 | 交互形式 | 产出 |
|---|---|---|
| egui | 每个选项渲染为可点击行（`ui.selectable_label`），点击 → `toggle_choice` → push `ChoiceSelect` | 完整闭环 |
| iced | 选项行加 `.on_press`（消息模式与现有 CheckToggle 一致，app.rs `UserAction` 补对应变体） | 完整闭环 |
| TUI | 沿用现有焦点/按键模型：ChoicePicker 进入可聚焦控件序列，Enter/Space 触发 `toggle_choice` → `ChoiceSelect` | 完整闭环 |
| web | SSR 字符串渲染：生成带 `data-a2ui-*` 标注的 checkbox/radio 组（`variant` 映射 input type），选中态来自 §3.4；宿主桥接层负责把 DOM 事件转成 `handle_user_event(ChoiceSelect)`（与现有 TextInput 桥接同模式） | 渲染 + 事件入口；DOM 侧桥接不在 Rust 范围 |

### 3.6 平台本地 UI 状态：明确豁免

iced 的 `checkbox_values` / `text_input_values` 本地缓存（受控组件模式：交互后本地立即回显，不等服务端回包）**保留且不要求其他平台效仿**。约束只有一条：**本地缓存未命中时的兜底解析必须走公共 helper**（`checkbox_checked` / `resolve_str_list`）。iced 现有测试 `test_checkbox_state_uses_component_id` 继续锁定缓存优先语义。ChoicePicker 在 iced 侧同样允许（也不强制）建本地选中集缓存。

### 3.7 非规范扩展的处置：冻结 + 记录

以下三处保留现状、不扩散，在 ARCHITECTURE.md 增补「规范扩展登记」小节逐条记录（键名、扩展方、理由）：

1. **CheckBox `checked` 回退键**（读：TUI/egui/web + helper；写：候选键第二位）——历史兼容。
2. **Image `width`/`height`**（egui/iced）——规范用 `variant` 尺寸档位，本仓库先于规范实现了像素尺寸；删除会破坏现有示例，规范 `variant` 档位落地时再评估收敛。
3. **Modal `title`**（web）——规范 Modal 无标题键；规范输入下该代码永不触发，无害。

**不新增任何扩展键**是本步之后的执行纪律。

### 3.8 P1：variant 对齐（低风险独立批次）

统一「识别集」，渲染表现允许平台降级（TUI 没有真密码框但至少要打码）：

| 组件 | 改动 |
|---|---|
| Button | iced 补 `primary`（主色样式）；TUI/egui 补 `borderless`（能力内降级：TUI 去边框字符 / egui `ui.link` 风格）；web 已齐 |
| TextField | TUI/web 补 `obscured`（TUI 渲染 `*` 打码；web `<input type="password">`）。`longText`/`number` 属功能增强，不在本步 |
| Text | TUI/egui/iced 补 `caption`（小一号/弱化样式，平台能力内） |

---

## 4. 迁移计划（每个 commit 红→绿→`cargo test --workspace` 全绿→fmt/clippy 无新增→提交）

优先级排序：D0-D2 是地基（core/renderer 公共层），D3 是 P0 中影响面最大的符合性修复，D4-D7 平台闭环，D8-D9 收尾。

| # | 内容 | 依据 | 红测试要点 |
|---|---|---|---|
| D0 | core：`OptionDecl` + `options_decl()`（对象/裸字符串双形态 + 宽容）；`prop_dynamic_str_list` | §3.3, §3.4 | 规范对象形态、裸字符串兼容、label 缺失退化、畸形项跳过、`{"path"}` 整体绑定形态四象限 |
| D1 | renderer：`resolve_str_list` + `toggle_choice` 纯函数 | §3.4, §3.5 | resolve 四象限对齐 `resolve_bool` 语义；toggle 单选替换/多选增删/variant 缺省 |
| D2 | renderer：`checkbox_checked` helper + `UserEvent::ChoiceSelect` + 写回分支（含 RendererCore 流水线贯通） | §3.2, §3.5 | helper 四象限（value 优先/checked 回退/绑定/缺省）；ChoiceSelect 写回数组；`handle_user_event` 端到端 |
| D3 | 四家 ChoicePicker **读取侧**迁移：`options_decl` + 选中态渲染（含 iced 补选中态） | §3.3, §3.4 | 每家：规范对象形态渲染出 label、选中态高亮/标记；裸字符串回归用例保持绿 |
| D4 | 四家 CheckBox 读取迁移到 `checkbox_checked`（TUI 行为升级在此发生） | §3.2 | TUI：`value` 绑定勾选用例（现状红）；egui/web 等价性回归；iced `checked` 回退用例 |
| D5 | egui + iced ChoicePicker 交互（点击 → toggle → ChoiceSelect → 写回 → 缓存失效） | §3.5 | 点击后 DataModel 更新 + 单选/多选语义 + iced 缓存失效断言 |
| D6 | TUI ChoicePicker 交互（焦点序列 + 按键） | §3.5 | 按键事件产出 ChoiceSelect；写回后重渲染标记更新 |
| D7 | web ChoicePicker 交互渲染（input 组 + data 标注）+ `handle_user_event(ChoiceSelect)` 入口 | §3.5 | HTML 含正确 input type（variant 映射）与选中态；事件入口写回 |
| D8 | P1 variant 对齐（Button/TextField/Text，见 §3.8） | §3.8 | 每平台每 variant 一个渲染断言；重点 `obscured` 打码 |
| D9 | ARCHITECTURE.md「规范扩展登记」+ 本文档标记已实施 | §3.7 | — |

依赖关系：D3-D7 依赖 D0-D2；D8 独立可并行；D5/D6/D7 相互独立。若需拆批次交付，**最小有价值集 = D0-D4**（规范消息从「渲染空列表/恒不勾选」变为正确渲染，交互后补）。

## 5. 风险

1. **`UserEvent` 新变体的下游 match**：四家渲染器与 RendererCore 对 `UserEvent` 的 match 若无通配臂会编译红——这是期望行为（编译器驱动找全消费点），D2 提交内一并补齐所有臂。
2. **TUI CheckBox 行为变化**：只声明 `value` 的规范消息从「恒不勾选」变为正确——严格说这是 bug 修复，但若有下游依赖旧错误行为会察觉差异。commit message 注明。
3. **iced 选中态渲染变化**：从忽略 `value` 到渲染选中标记——同上，向规范收敛的可见变化。
4. **web 的交互边界**：Rust 侧只能做到「渲染可交互标记 + 提供事件入口」，DOM 事件桥接在宿主侧。D7 验收标准要写清楚这条边界，避免误判「web 没做完」。

## 6. 明确不做（并说明为什么）

- **删除 `checked` / Image 尺寸键 / Modal title 扩展**：CLAUDE.md 禁删 + 有既有使用者，冻结登记即可（§3.7）。
- **ChoicePicker 的 `displayStyle`/`filterable`/`label`、FunctionCall 求值**：功能增强而非差异修复；FunctionCall 在全部 Dynamic 解析路径上均未实现，属独立专项。
- **`checks`（Checkable）体系**：五个输入组件共有的独立大项。
- **iced 本地缓存机制统一**：平台受控组件模式的合理差异（§3.6）。
- **web-react TS 客户端对齐**：不在 Rust workspace；其 store.ts 的 ChoicePicker 行为需要另行核对。

---

## 实施记录（2026-07-08）

| 步 | Commit | 备注 |
|---|---|---|
| D0 | `7ca3b87` | 按设计 |
| D1 | `964e299` | 按设计 |
| D2 | `74e985b` | `UserEvent` 非 `non_exhaustive`，但四家 match 均有通配臂，无编译波及 |
| D3 | `e6f547a` | 计划外新增公共层 `choice_options`/`choice_selected`/`ChoiceOption`（label 求值与选中集解析一次裁决，符合 §3.1 原则）|
| D4 | `25caf96` | 三家私有旧 bool 解析函数按禁删约定加 `#[allow(dead_code)]` 保留 |
| D5 | `8d01158` | egui/iced widget 补 `variant` 字段承载 toggle 语义 |
| D6 | `00d17f7` | TUI 交互形态定为：Left/Right 移动选项游标（`choice_cursors` 平台本地状态）、Enter/空格切换、渲染 ▸ 游标标记 |
| D7 | `c86ec4c` | web SSR 从 `<select multiple>` 改为 fieldset + radio/checkbox 组（可见行为变化）|
| D8 | `4c9ba05` | egui/iced 纯视觉样式经 headless 冒烟覆盖（Element 不可内省），TUI/web 有输出断言 |
| D9 | 本 commit | ARCHITECTURE.md 增设「规范扩展登记」（4 项：checked 回退、裸字符串 options、Image 尺寸键、Modal title）|

与设计的偏离：
1. D3 的公共 helper（`choice_options`/`choice_selected`）为计划外新增——设计原文只要求「options_decl + 选中态渲染」，实施中发现 label 求值逻辑若留在四家会重新四写一遍，遂上提公共层。
2. 选中集读取从 `prop_str_list`（逐项过滤）切到 `prop_dynamic_str_list`（形态不符整体 None）后，**字面量数组混入非字符串项**的畸形输入从「过滤后渲染」变为「按空选中集处理」——仅影响畸形输入，规范合法消息无差异。**已于后续 commit 修复**：`choice_selected` 在类型化解析失败时回退 `prop_str_list` 的逐项过滤，声明侧与数据模型侧宽容度一致。
3. TUI Button/子组件渲染区域重叠导致帧缓冲断言不可靠（既有布局 quirk），D8 的 borderless 断言改为对 `button_display` 纯函数的单测。
