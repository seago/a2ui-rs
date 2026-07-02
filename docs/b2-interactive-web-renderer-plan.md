# B2 交互式 Web 渲染器实现计划（TS/React + 可插拔 ComponentKit）

## 背景（为什么做）

现有 `a2ui-renderer-web`（Rust）是**静态派**：把组件树拼成一次性 HTML 字符串，没有交互、没有 action 回传、没有响应性——只完成了「渲染」半个闭环。目标是**可交互的 Agent UI**，且要真正使用 shadcn，并保留将来切换到 MUI / Ant Design 等其他 React 组件库的能力。

因此新增一条独立的**交互派**渲染器（B2），与静态派互补、不替代。`ARCHITECTURE.md` 第 100 行起已记录两派分工，本计划落地交互派。

### 已确认的决策
1. **形态**：独立 TS/React 前端项目（不是 cargo crate）。
2. **核心层**：纯 TS 重写协议核心/响应性引擎（零 WASM），用**共享协议测试向量**让 Rust 核心与 TS 核心同跑同一份用例，防止两份逻辑走样。
3. **组件库**：两层结构「协议核心 + 可插拔 ComponentKit」，shadcn 为首个 kit，核心层与 kit 解耦以支持切换。
4. **首里程碑**：端到端活骨架（少数组件打通完整交互闭环），再横向补齐。

## 架构总览

```
Agent(后端) ──ServerEnvelope──▶ [WS 服务端](新增, Rust) ──▶ 浏览器 B2 ──┐
                                                                      │ 三层(TS)
   ▲                                                                  ▼
   └───────────── ClientEnvelope(action) ◀── [WS 服务端] ◀── ① 传输/协议核心层(纯 TS, 无 React)
                                                              ② 渲染核心(React, 库无关)
                                                              ③ ComponentKit(shadcn, 可切换)
```

- **① 协议核心层（纯 TS，无 React）**：TS 版协议类型 + 消息解析 + `SurfaceStore`（组件森林、Data Model/JSON Pointer、路径解析含 `@index` 与集合作用域、依赖图响应性、formatString、函数调度含 `callableFrom` 校验）。对齐 Rust `a2ui-renderer` 的模块语义。
- **② 渲染核心（React，库无关）**：`A2UIProvider`（持有 store + 当前 kit）、`Surface`、`useSurface`/`useBinding` hooks；遍历组件树，为每个节点从 kit 查对应 React 组件并传**归一化 props**；串联事件 → 写回 Data Model → 发 `action` → 收 `actionResponse` 写回 `responsePath`。
- **③ ComponentKit（唯一可切换层）**：把 18 个 A2UI 组件类型映射到具体 React 组件库。props 契约由 A2UI Catalog schema 定义，不由任何库定义；库不支持的能力在 kit 内优雅降级；未知组件渲染占位符。

## 仓库布局

| 路径 | 内容 | 改动类型 |
|------|------|----------|
| `clients/web-react/` | 全新 TS/React 前端项目（Vite + TS + React + Tailwind + shadcn + Vitest） | 新建 |
| `clients/web-react/src/core/` | ① 协议核心层（纯 TS） | 新建 |
| `clients/web-react/src/react/` | ② 渲染核心（Provider/Surface/hooks/tree-walker） | 新建 |
| `clients/web-react/src/kits/shadcn/` | ③ shadcn ComponentKit | 新建 |
| `crates/a2ui-transport/src/ws_server.rs` | Rust WS **服务端**（现有 `websocket.rs` 仅客户端连接） | 新增 |
| `crates/a2ui-transport/examples/serve_demo.rs` | 演示 bin：向浏览器推送一个 demo surface 并接收 action | 新建 |
| `tests/conformance/*.json` | 共享协议测试向量（输入消息流 → 期望解析后状态），Rust 与 TS 同跑 | 新建 |

> `a2ui-renderer-web`、`a2ui-core`、`a2ui-renderer` **不改**。

## 技术选型（默认，若有偏好可调）

- 构建：**Vite + React + TypeScript**
- 测试：**Vitest + React Testing Library**（对齐仓库 TDD 强制要求，红→绿→重构）
- 样式/组件：**Tailwind + shadcn**，按官方安装文档
- 传输：**WebSocket 优先**（SSE 留后续）
- 包管理：npm（可换 pnpm）

## 分阶段实现（每步 TDD：先红后绿再重构）

### 里程碑 M1 — 端到端活骨架
目标：`Text / Button / TextField / Card` 四个组件打通**完整交互闭环**（服务端推送 → 渲染 → 输入/点击 → action 回传 → actionResponse 写回 Data Model → 局部重渲染）。每层只切最小片。

- **M1.0 脚手架**：在 `clients/web-react/` 建 Vite+React+TS 工程，接入 Vitest+RTL、Tailwind+shadcn；跑通一个空测试。
- **M1.1 协议核心（TS，核心中的核心）**：
  - TS 协议类型：`ServerEnvelope`/`ClientEnvelope`/`Component`/`DynamicValue`/`ChildList`/`Action`（依 ARCHITECTURE.md「关键数据结构」章节）。
  - `DataModel`：JSON Pointer `applyPointer`/`resolvePointer`（upsert + null 删除），对齐 `crates/a2ui-renderer/src/data_binding.rs`、`path_resolver.rs`。
  - `SurfaceStore`：组件森林（flat map + 建树 + 缺失引用占位）、Data Model、依赖图响应性（对齐 `dependency_graph.rs`）。
  - `formatString` + 函数调度（`callableFrom` 校验，对齐 `function_dispatcher.rs`、`format_string.rs`）——M1 仅接骨架所需最小函数集。
  - **共享测试向量**：在 `tests/conformance/` 写 JSON 用例，TS（Vitest）与 Rust（cargo test）各写一个 runner 消费同一批用例。
- **M1.2 传输层**：
  - Rust：`ws_server.rs` 新增 WS 服务端，参照现有 `crates/a2ui-transport/src/websocket.rs` 与 `transport.rs` 的 `Transport` trait 语义（推 `ServerEnvelope`、收 `ClientEnvelope`）；TDD 用 tokio 起监听、连接、收发。
  - TS：WS 客户端（含重连），把收到的消息喂给 `SurfaceStore`，把 action 发回。
- **M1.3 渲染核心（React，库无关）**：`A2UIProvider` + `Surface` + `useBinding`；tree-walker 按组件类型查 kit；用一个**mock kit** 做 TDD（断言点击 Button 触发 action、TextField 输入写回 Data Model）。
- **M1.4 shadcn kit（骨架四组件）**：`Text/Button/TextField/Card` → shadcn，RTL 断言渲染与交互；变体枚举映射（如 Button primary/default/borderless）。
- **M1.5 端到端打通**：起 `serve_demo` bin + Vite dev，手动跑一遍 prompt-generate-validate 闭环，确认 action 往返与写回生效。

### 里程碑 M2 — 补齐 Basic Catalog 18 组件
按 ARCHITECTURE.md 组件清单，把剩余组件的**归一化 props 契约**与 **shadcn kit 实现**逐个补齐（每个组件一轮 TDD）：Image/Icon/Video/AudioPlayer/Row/Column/List/Tabs/Modal/Divider/CheckBox/ChoicePicker/Slider/DateTimeInput。含 `ChildList` 模板模式（template + path 动态生成 + 集合作用域 `@index`）。

### 里程碑 M3 — 验证「可切换组件库」
新增一个最小的第二 kit（例如 MUI 或纯 HTML kit，只做 3~4 个组件），用 `<A2UIProvider kit={...}>` 切换，断言：切换后状态（Data Model/输入值）保留、核心层零改动。以此证明契约缝真的成立。

## 验证方式（端到端）

- **Rust**：`cargo test --workspace`（含新 WS 服务端单测 + conformance runner）全绿。
- **TS**：`cd clients/web-react && npm test`（Vitest：核心层单测、渲染核心 + kit 的 RTL 测试、conformance runner）全绿。
- **共享向量**：同一份 `tests/conformance/*.json` 在 Rust 与 TS 两侧断言一致的解析后状态——这是防「两份逻辑走样」的关键闸门。
- **端到端手测**：`cargo run -p a2ui-transport --example serve_demo` 起 WS 服务端 → `npm run dev` 打开前端 → 在页面点击/输入，观察 action 回传与 `actionResponse` 写回触发的重渲染。
- **切换验证**（M3）：页面上切换 kit，确认视觉换库、状态保留。

## 约束与提醒

- 遵守仓库 TDD 强制流程（红→绿→重构），Rust 侧不得先写实现再补测试；TS 侧同样先写 Vitest/RTL 测试。
- 不改 `a2ui-core` 消息类型；不删除现有函数/类型。
- 功能改动与格式化改动分开提交。
- M1 完成即可交付一个可演示的活骨架；M2/M3 可分批推进。
