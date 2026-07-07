# 架构返工 · 第 0 步：客户端事件 Wire 格式统一（决策文档）

状态：已对照规范复核（a2ui.org/specification/v1.0-a2ui，2026-07-07 抓取）
关联：代码审查报告「横切主题：同类逻辑多份复制且已发散」；[第 1 步设计](refactor-step1-renderer-core.md)（本决策由公共层实现落地）

## 1. 问题

同一次用户交互，五份实现产出三种互不兼容的消息格式；且经与规范逐条核对，**五份实现全部存在规范符合性缺口**（详见 §2 各决策的「规范依据」）。现状对照：

| 维度 | TUI / Web / egui | iced | web-react | 规范要求 |
|---|---|---|---|---|
| 文本输入/复选框是否发消息 | 发（`input`/`toggle`） | 发（`text_input`/`check_toggle`） | 不发 | **不发**（被动变更不触发网络请求） |
| 组件声明的 `action` 属性 | 忽略（一律合成 `click`） | 忽略 | 用扁平 `action.name`（偏差） | 用 `action.event.{name,...}`（嵌套） |
| 来源组件 | context 的 `source`/`component` 键 | `sourceComponentId` 字段 | `sourceComponentId` 字段 | `sourceComponentId` 字段，**必填** |
| `surfaceId` | 恒 `""` | 部分填 | 填 | **必填** |
| `timestamp` | 无 | 无 | 无 | **必填**（ISO 8601） |
| `responsePath` 是否上线路 | 是（字段存在即发） | 是 | 是 | **否**（客户端本地语义） |
| context 值类型 | 字符串化 | 原生 | 原生 | 原生（规范示例 `"isSubscribed": true`） |
| dataModel 附带位置 | context `"dataModel"` | context `"data_model"` | 信封 `metadata` | **transport metadata**（位置由 binding 定义） |
| dataModel 取自哪个 surface | 第一个 enabled 的 surface | 同左 | 事件所属 surface | **仅创建该 surface 的服务端**（定向投递） |

## 2. 决策（每条附规范依据）

### D1. 只有声明了 server action 的组件交互才发送 action 消息

> 规范 · action 消息：“This message is sent when a user interacts with a component that has a server action defined (such as a Button).”

- 组件声明格式（规范 · Server actions）：`action: { event: { name, context?, wantResponse?, responsePath? } }`——注意是**嵌套 `event` 键**，web-react demo 的扁平 `action: {name}` 是偏差，本仓库 Rust 侧按规范解析。
- `action: { functionCall: {...} }` 为本地函数调用，不产生网络消息。
- **无声明 action 的组件交互不发送任何消息**。原先的合成 `click` 取消。

### D2. 取消输入类合成事件（input/toggle/slider_change 全部废除）

> 规范 · Client to server updates：“Passive data changes (like typing in a text field) do not trigger a network request on their own; they simply update the local state, which will be sent with the next action.”

文本输入/复选框/滑块变更只做**本地数据模型写回**（上一批次已实现的 `write_back_user_event`），不再发送消息。最新状态随下一次声明式 action 的 metadata 到达服务端。四个 Rust 渲染器的合成事件全部移除；原「统一事件名」的议题随之消失。

### D3. action 消息字段按规范补齐

> 规范 · action 消息 Properties：`name`、`surfaceId`、`sourceComponentId`、`timestamp` 均 **required**；`context` required（绑定求值后的对象）；`wantResponse` optional；`actionId`（`wantResponse=true` 时 required）。

- `surfaceId`：经 `forest.surface_of(component_id)` 反查；失败（组件不属于任何 surface）丢弃事件并 `tracing::warn`。
- `sourceComponentId`：必填。`a2ui-core` 的 `ActionMessage.source_component_id` 当前是 `Option`——**改为必填**（C0）。
- `timestamp`：`ActionMessage` 当前缺此字段——**新增**（C0）。生成方式待定：引入 `time`/`chrono` 依赖（须按 CLAUDE.md 先确认）或基于 `SystemTime` 手写 UTC ISO 8601 格式化（推荐后者，零依赖，`function_dispatcher` 已有日期处理可参考）。
- `context`：声明中的 `DynamicValue`（path 绑定 / functionCall）对该 surface 数据模型求值后的原生 JSON 值。

### D4. `responsePath` 不上线路

> 规范 · Server actions：“responsePath (string, optional): A JSON Pointer path **in the local data model** where the response value should be saved.” action 消息的 Properties 列表中**没有** responsePath。

`responsePath` 是客户端本地语义：构造消息时公共层自动 `register_pending_response(action_id, surface_id, response_path)`（上一批次已改造为记录 surface_id），消息本体不携带。`a2ui-core` 的 `ActionMessage.response_path` 字段保留（含 `skip_serializing_if`），但公共层永不填充；web-react 发送 responsePath 属偏差，后续对齐。

### D5. dataModel 经信封级 `metadata` 附带，且仅限事件所属 surface

> 规范 · sendDataModel：“the client will send the full data model of this surface **in the metadata** of every message … (via the Transport's metadata mechanism)”；“The data model is included in the transport metadata, **tagged by its surfaceId**”；“**Targeted Delivery**: The data model is sent exclusively to the server that created the surface.”

- 规范把 metadata 的确切位置留给 transport binding。**本仓库 WS/JSONL binding 定义为信封级字段**（与 web-react 一致）：`{ "version": "v1.0", "action": {...}, "metadata": { "surfaceId": "...", "dataModel": {...} } }`。
- `a2ui-core` 的 `ClientEnvelope` 需增加可选 `metadata`（C0）——当前 `deny_unknown_fields` 会拒收 web-react 的合法信封，这是已存在的互操作 bug。
- 附带条件：**事件所属 surface**（`surface_of` 反查）的 `sendDataModel == true`。废除「取第一个 enabled surface」（违反规范定向投递要求，且跨 surface 越权泄漏）。
- context 中的 `"dataModel"`/`"data_model"` 键全部废除。

### D6. actionResponse 的 wire 格式按规范修正（顺带发现的 a2ui-core 偏差）

> 规范 · actionResponse Properties：`actionId`（required，**信封层**、与 actionResponse 平级）；`actionResponse`（required）内为 `value`（成功）**或** `error: {code, message}`（失败），“Exactly one of value or error must be present.”

即规范 wire 是：

```jsonc
{ "version": "v1.0", "actionId": "a-123", "actionResponse": { "value": {...} } }
{ "version": "v1.0", "actionId": "a-123", "actionResponse": { "error": { "code": "E1", "message": "..." } } }
```

而 `a2ui-core` 当前把 `actionId` 放在 payload 内、success 直接 flatten、error 平铺 `{code,message}`——**与规范冲突**（违反 CLAUDE.md「不得引入与规范冲突的类型定义」）。这同时裁决了审查发现的 web-react 两套实现分歧：`store.ts` 读 `envelope.actionId` 是对的，`core/messages.ts` 读 payload 内 actionId 是错的。修正纳入 C0。

### D7. KeyPress 语义

Tab/Up/Down 焦点导航为渲染器本地行为（不产消息）；Enter/空格 = 对焦点组件的一次交互，等价 Click 走 D1（有声明 action 才发）。公共层不接收 KeyPress。

## 3. 规范样例（本 binding 下的完整信封）

```jsonc
// Button 声明：
// { "id":"submit_button", "component":"Button", "child":"lbl",
//   "action": { "event": { "name":"submit_form",
//     "context": { "isSubscribed": {"path":"/contact/subscribe"} },
//     "wantResponse": true, "responsePath": "/result" } } }
//
// 用户点击后客户端发送（surface s1 开启 sendDataModel）：
{
  "version": "v1.0",
  "action": {
    "name": "submit_form",
    "surfaceId": "s1",
    "sourceComponentId": "submit_button",
    "timestamp": "2026-07-07T08:57:23Z",
    "context": { "isSubscribed": true },
    "wantResponse": true,
    "actionId": "a-123"
  },
  "metadata": { "surfaceId": "s1", "dataModel": { "contact": { "subscribe": true } } }
}
// responsePath 未上线路：客户端本地登记 a-123 → (s1, /result)，
// 收到 actionResponse 后写回 /result。
```

## 4. 迁移影响

| 影响面 | 内容 |
|---|---|
| a2ui-core（C0） | `ClientEnvelope` 加 `metadata`（D5）；`ActionMessage` 加必填 `timestamp`、`source_component_id` 改必填（D3）；`ActionResponse` wire 格式修正（D6）。均为协议符合性修复，按 CLAUDE.md 同步更新全部下游测试 |
| 四个 Rust 渲染器 | `handle_user_event` 移入公共层；合成事件（click/input/toggle/slider_change）全部移除（D1/D2）——**demo 与示例（serve_demo 等）依赖收到合成 action 推进流程的，需改为声明式 action** |
| 服务端消费方 | 破坏性：不再收到输入类事件；action 一律来自组件声明；dataModel 从 context 移到 metadata |
| web-react | `metadata` 格式即其现状（D5 无需改）；后续偏差项：扁平 action 声明（D1）、responsePath 上线路（D4）、缺 timestamp（D3）、`core/messages.ts` 的 actionId 位置（D6）——**不在本批范围**，记录待办 |
| 测试 | 四渲染器 handle_user_event 断言重写为本规范 |

## 5. 已解决的原未决项

1. ~~D6/metadata 需规范复核~~ → 已复核确认（§2 D5）。
2. ~~输入类合成事件是否长期保留~~ → 规范明确不发送，废除（§2 D2）。
