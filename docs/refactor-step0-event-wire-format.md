# 架构返工 · 第 0 步：客户端事件 Wire 格式统一（决策文档）

状态：待评审
日期：2026-07-07
关联：代码审查报告「横切主题：同类逻辑多份复制且已发散」；[第 1 步设计](refactor-step1-renderer-core.md)（本决策由公共层实现落地）

## 1. 问题

同一次用户交互，五份实现产出三种互不兼容的消息格式，服务端无法用一套逻辑消费：

| 维度 | TUI / Web / egui | iced | web-react 参考客户端 |
|---|---|---|---|
| 文本输入事件名 | `input` | `text_input` | 不发合成事件（只写数据模型） |
| 复选框事件名 | `toggle` | `check_toggle` | 同上 |
| 来源组件 | context 的 `source`/`component` 键 | `sourceComponentId` 字段 | `sourceComponentId` 字段 |
| `checked`/`value` 类型 | 字符串 `"true"`、`"42.5"` | 原生 `Bool`/`Number` | — |
| dataModel 附带位置 | context 的 `"dataModel"` 键 | context 的 `"data_model"` 键 | 信封级 `metadata: {surfaceId, dataModel}` |
| `surfaceId` 字段 | 恒为 `""`（违反协议必填语义） | sendDataModel 命中时填，否则 `""` | 始终填写 |
| 组件声明的 `action` 属性 | 忽略（一律合成 `click`） | 忽略 | 使用声明的 `action.name`/`context`/`wantResponse` 等 |
| dataModel 取自哪个 surface | 「第一个 enabled 的 surface」（HashMap 序，跨 surface 泄漏） | 同左 | 事件所属 surface |

现状代码位置：`tui_renderer.rs:600-731`、`web_renderer.rs:644-761`、`gui_renderer.rs:506-621`、`iced_renderer.rs:419-501`、`clients/web-react/src/core/store.ts:555-601`。

## 2. 决策

### D1. 声明式 action 优先，合成事件作为降级

Click/activate 交互**优先读取组件声明的 `action` 属性**（协议本意，web-react 参考客户端已如此）：

```jsonc
// Agent 声明（web-react demo/surface.ts:112 现有格式）
{ "id": "bp", "component": "Button", "child": "bp_l", "action": { "name": "submit" } }
```

- 有声明：`ActionMessage.name` 取 `action.name`；`action.context` 中的 `DynamicValue`（含 path 绑定）对该 surface 的数据模型求值后放入 `context`；`wantResponse`/`responsePath`/`actionId` 原样透传。
- 无声明：降级合成 `name = "click"`（保持现有 Rust demo/示例可用）。

**理由**：`ActionMessage` 的 `responsePath`/`actionId` 字段只有在 action 由 Agent 声明时才有来源；四个 Rust 渲染器忽略声明是协议符合性缺口，不是格式偏好问题。

### D2. 合成事件名规范

`click`（点击，无声明 action 时）· `activate`（键盘激活，语义同 click）· `input`（文本输入）· `toggle`（复选框）· `slider_change`（滑块）。

采用 TUI/Web/egui 的多数派命名，**iced 迁移**（`text_input`→`input`、`check_toggle`→`toggle`）。这是对 iced 消费方的破坏性变更，在第 1 步迁移 commit 中显式声明。

### D3. `surfaceId` 必填，由组件反查

`surfaceId` 一律通过 `forest.surface_of(component_id)` 反查获得。反查失败（组件不属于任何 surface）说明事件源已失效——**丢弃事件并 `tracing::warn`，不再发送 `surfaceId: ""` 的消息**。

### D4. 来源组件走 `sourceComponentId` 字段

协议字段本就存在（`client_to_server.rs:43`），web-react 也在用。**废除** context 中的 `"source"`/`"component"` 键。

### D5. context 值使用原生 JSON 类型

`value`（文本）→ `String`；`value`（滑块）→ `Number`；`checked` → `Bool`。TUI/Web/egui 迁移掉字符串化编码。

### D6. dataModel 附带位置：信封级 `metadata`（⚠ 需对照规范复核）

**推荐**：采用 web-react 的信封级格式，`a2ui-core` 的 `ClientEnvelope` 增加可选 `metadata` 字段：

```jsonc
{
  "version": "v1.0",
  "action": { "name": "submit", "surfaceId": "s1", "sourceComponentId": "bp" },
  "metadata": { "surfaceId": "s1", "dataModel": { "form": { "username": "alice" } } }
}
```

**理由**：(a) `context` 的语义是「Agent 在 action 声明中定义的上下文」，传输层附带的快照混入其中污染语义；(b) web-react 参考客户端已是此格式，且当前 Rust 侧 `ClientEnvelope` 带 `deny_unknown_fields`，**会直接拒绝 web-react 发来的信封**——这是一个已存在的互操作 bug，本决策顺带修复。

**代价**：需要修改 `a2ui-core` 消息类型（按 CLAUDE.md 要求同步更新所有下游测试）。⚠ 无法访问 a2ui.org 规范站点核实 `metadata` 是否为规范定义字段——落地前需人工对照规范确认；若规范用其他字段名/位置，以规范为准，本决策仅锁定「不放 context」这一点。

### D7. dataModel 只取事件所属 surface

附带条件：**事件所属 surface**（`surface_of` 反查结果）的 `sendDataModel == true`。废除「遍历取第一个 enabled surface」——这既是不确定性来源，也是跨 surface 数据越权泄漏（审查报告 web #20）。

### D8. KeyPress 不进公共格式

- Tab/Up/Down 焦点导航是渲染器本地行为，不产生消息（现状保持）。
- Enter/空格激活：由渲染器把焦点组件解析为 `Click { component_id }` 后走 D1 流程，事件名用 `activate`。公共层不接收 KeyPress。

### D9. pending_response 自动登记

消息含 `wantResponse: true` 且 `actionId` 时，公共层构造消息的同时自动 `register_pending_response(action_id, surface_id, response_path)`（web-react store.ts:589 已如此）。手动注册 API 保留。

## 3. 规范样例

```jsonc
// 文本输入（合成事件，surface s1 开启 sendDataModel）
{
  "version": "v1.0",
  "action": {
    "name": "input",
    "surfaceId": "s1",
    "sourceComponentId": "username_field",
    "context": { "value": "alice" }
  },
  "metadata": { "surfaceId": "s1", "dataModel": { "form": { "username": "alice" } } }
}

// 复选框（未开启 sendDataModel）
{
  "version": "v1.0",
  "action": {
    "name": "toggle",
    "surfaceId": "s1",
    "sourceComponentId": "agree_cb",
    "context": { "checked": true }
  }
}

// 声明式 action 的按钮点击（含响应回写）
{
  "version": "v1.0",
  "action": {
    "name": "submit",
    "surfaceId": "s1",
    "sourceComponentId": "bp",
    "wantResponse": true,
    "responsePath": "/result",
    "actionId": "a-123"
  }
}
```

## 4. 迁移影响

| 影响面 | 内容 |
|---|---|
| a2ui-core | `ClientEnvelope` 增加可选 `metadata` 字段（D6）；`ActionMessage` 不变 |
| 四个 Rust 渲染器 | `handle_user_event` 全部改走公共层（第 1 步），事件名/值类型/字段随之统一 |
| iced 消费方 | 事件名变更（破坏性，D2） |
| TUI/Web/egui 消费方 | `checked`/`value` 类型变更（字符串→原生，D5）；`dataModel` 从 context 移到 metadata（D6）；`surfaceId` 从 `""` 变为实际值（D3） |
| web-react | 无需改动（D6 采用其格式）；后续可对齐 D1 的合成事件补充 |
| 测试 | 四个渲染器 handle_user_event 相关断言全部更新为本规范 |

## 5. 未决项

1. **D6 需规范复核**：`metadata` 字段名与位置以 a2ui.org v1.0 规范为准。
2. 输入类合成事件（input/toggle/slider_change）是否应该长期存在：web-react 不发送它们（数据经 sendDataModel 随下一次 action 到达）。本次保留 Rust 侧行为，是否裁剪留给协议符合性专项。
