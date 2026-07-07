# 架构设计

本文档描述 `a2ui-rs` 的架构设计与技术选型，作为实现的完整蓝图参考。

## 协议背景

`a2ui-rs` 是 [A2UI (Agent to UI) Protocol v1.0](https://a2ui.org/specification/v1.0-a2ui/) 的 Rust 实现。

A2UI 是一个基于 JSON 的流式 UI 协议，Agent（服务端）通过 JSON 消息流向 Renderer（客户端）描述和更新 UI。核心哲学：UI 结构与应用数据分离，支持渐进式渲染——渲染器每收到一条消息就解析并增量构建 UI，无需等待完整 payload。

v1.0 版本（2025-11-20 创建，2026-06-08 更新）相比 v0.9 的主要变化：
- 双向 RPC：`actionResponse`（服务端响应客户端 action）、`callFunction` / `functionResponse`（服务端调用客户端注册函数）
- 单消息 UI 实例化：`createSurface` 可内嵌完整的组件树和 data model
- 解耦品牌化：用可扩展的 `surfaceProperties` 替代硬编码主题属性
- 增强的 Catalog Schema：函数定义改为对象映射（O(1) 查找），支持 `$schema` / `$id`
- 严格标识符规范：Unicode UAX #31 命名规则，`@` 命名空间保留给系统

## 总体架构

```
┌──────────┐   A2UI JSON 流    ┌──────────────────┐   ┌────────────────────┐
│  Agent    │─────────────────▶│  Transport 传输层  │──▶│  Protocol Core 核心 │
│（服务端） │  createSurface…   │ WS / SSE / JSONL  │   │ 消息解析·状态机·校验 │
└──────────┘◀─────────────────└──────────────────┘   └─────────┬──────────┘
              action 回传                                        │
        ┌──────────────┬──────────────┬──────────────┬──────────┴──────────┐
        ▼              ▼              ▼              ▼                     ▼
  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────────┐   ┌──────────────────┐
  │ TUI       │   │ egui GUI  │   │ iced GUI  │   │ Web 静态派   │   │ Web 交互派（B2）  │
  │（ratatui）│   │（eframe） │   │          │   │ HTML 字符串  │   │ TS/React +        │
  └──────────┘   └──────────┘   └──────────┘   └─────────────┘   │ 可插拔 ComponentKit│
                                                                 │ shadcn / html …   │
   └──── Rust 渲染器：共用 a2ui-renderer 核心 + a2ui-transport ────┘   └──────────────────┘
                                                                    浏览器端独立 TS 实现
```

协议与渲染实现完全分离。Transport Layer 将 JSON 消息流送入 Protocol Core，Protocol Core 维护 Surface 状态并驱动各渲染器。**Rust 系渲染器**（TUI / egui / iced / Web 静态派）采用「**RendererCore + 平台 widget 映射**」结构：协议状态（组件森林、数据绑定、依赖图、生命周期状态机、LRU、pending 响应）与六类消息的处理流水线全部收敛在 `a2ui-renderer` 的 `RendererCore`，各平台渲染器组合该核心、只保留平台特有部分（TUI 的焦点管理与帧绘制、egui 的纹理缓存、iced 的 RefCell 渲染缓存、Web 的 HTML 构建），消息处理返回的 `CoreEffects` 告知平台失效哪些渲染缓存；**Web 交互派（B2）** 是浏览器端独立的 TS/React 实现，靠同一份协议类型 + 一致性向量与 Rust 侧对齐（见「[跨语言一致性向量](#跨语言一致性向量防漂移)」）。

## Workspace Crate 规划

| Crate | 职责 | 关键依赖 |
|-------|------|----------|
| `a2ui-core` | 协议类型定义、消息枚举、JSON Schema 解析、状态机 | `serde`, `serde_json`, `thiserror` |
| `a2ui-transport` | 传输层抽象 trait + JSONL / WebSocket 客户端 / WebSocket 服务端（`ws_server`）+ 能力协商。AG-UI / A2A / MCP / SSE 为规划中的绑定 | `tokio`, `tokio-tungstenite`, `async-trait` |
| `a2ui-renderer` | `Renderer` trait、**`RendererCore`（四渲染器共享的协议状态 + 消息处理流水线，接入生命周期状态机，经 `CoreEffects` 通知平台缓存失效）**、组件森林、数据绑定引擎、路径解析、函数调度、Catalog 注册表、DependencyGraph、SurfaceLru、CustomComponentRegistry | `a2ui-core`, `serde_json` |
| `a2ui-renderer-tui` | TUI 渲染器实现 | `ratatui`, `crossterm`, `a2ui-renderer` |
| `a2ui-renderer-egui` | 桌面渲染器（egui + eframe 即时模式 GUI） | `egui`, `eframe`, `a2ui-renderer` |
| `a2ui-renderer-iced` | 桌面渲染器（iced 保留模式 GUI） | `iced`, `a2ui-renderer` |
| `a2ui-renderer-web` | Web 渲染器实现（静态派：服务端把组件树渲染为 HTML 字符串，非交互）；交互派见下方独立 TS/React 项目 | `a2ui-core`, `a2ui-renderer` |
| `a2ui-cli` | 命令行入口，集成 transport + renderer | `clap`, `a2ui-transport`, `a2ui-renderer-*` |

> **serde_json 隔离约束**：`a2ui-core` 与 `a2ui-renderer` 是仅有的两个允许直接依赖 `serde_json` 的 crate（白名单，CI 以 `scripts/check-serde-isolation.sh` 守护）。core 是协议任意 JSON 字段（DataModel / Catalog schema / `FunctionResponse.value` / 信封 metadata）的法定居所，并经 re-export（`Value`、`json!`）与类型化访问器（`prop_*` 系列、`ActionDecl`/`ChildrenDecl`/`TabDecl`/`StyleDecl` 视图、`Component::from_value`/`from_json`）构成下游唯一入口；renderer 是数据绑定引擎，`DataBinding`/`PathResolver`/函数调度对任意 JSON 的操作是其职责本体。四个平台渲染器、transport、cli 的 Cargo.toml 不得出现 `serde_json`。设计与迁移记录见 [docs/refactor-step2-serde-isolation.md](docs/refactor-step2-serde-isolation.md)。

> **Web 交互派（B2）** 不是 Rust crate，而是独立的 TS/React 项目 `clients/web-react`：浏览器端「协议核心（纯 TS）+ 可插拔 ComponentKit」两层架构，内置 shadcn/ui 与纯 HTML 两套可热切换的 kit，传输用 `wsClient` / `sseClient`。它与 Rust 侧靠 `a2ui-core` ↔ 前端 `src/contracts` 的同一份协议类型 + 一致性向量对齐。

> **Basic Catalog 捆绑方式**：A2UI v1.0 的 Basic Catalog JSON 文件（18 组件 + 14 函数）作为 workspace 资源文件捆绑，路径约定为 `crates/a2ui-core/assets/catalogs/basic/catalog.json`。渲染器启动时自动加载默认 Catalog，也支持从 URI 加载自定义 Catalog。

### 模块划分（以 `a2ui-core` 为例）

```
a2ui-core/src/
├── lib.rs                  # 公共 API 导出
├── message/
│   ├── mod.rs
│   ├── envelope.rs         # Envelope 消息分发（oneOf 解析）
│   ├── server_to_client.rs # createSurface / updateComponents / updateDataModel / deleteSurface / actionResponse / callFunction
│   └── client_to_server.rs # action / functionResponse / error
├── component/
│   ├── mod.rs
│   ├── component.rs        # Component 结构（id, component type, properties）
│   ├── catalog.rs          # Catalog 结构（components map, functions map, surfaceProperties）
│   └── child_list.rs       # ChildList 处理（array / object template）
├── datamodel/
│   ├── mod.rs
│   └── model.rs            # Data Model（serde_json::Value 的包装，提供 upsert / delete 操作）
├── schema/
│   ├── mod.rs
│   ├── common_types.rs     # 内联 common_types.json 的类型定义
│   ├── server_to_client.rs # 内联 server_to_client.json 的验证逻辑
│   └── catalog_schema.rs   # Catalog 的 JSON Schema 验证
└── state/
    ├── mod.rs
    └── surface_state.rs    # Surface 状态机（Created / Active / Deleted）
```

### 各渲染器设计要点

**TUI（终端界面）**
- ratatui 的布局系统直接映射 A2UI 的 `Row` / `Column`，`List` 映射为 `List` widget（带虚拟滚动）
- `Text` / `Button` / `TextField` / `CheckBox` / `Slider` / `ChoicePicker` 直接映射为 ratatui 对应 widget
- `Image` / `Video` / `AudioPlayer` 在 TUI 中降级为文本描述（终端无法渲染多媒体）
- `Icon` 映射为 Unicode 字符或 ASCII 符号
- `Tabs` 映射为 Tab 标题栏 + 内容区切换
- `Modal` 映射为覆盖层（ratatui 的 `Popup` 或手动管理光标区域）
- 方向键 / Tab 为渲染器本地焦点导航（不产生消息）；Enter / 空格由焦点管理器转译为焦点组件的 `Click` 交给 `RendererCore`——只有声明了 `action.event` 的组件才产生 `action` 消息
- 优势：无依赖外部服务，适合本地 Agent 直连（STDIN/STDOUT 或 WebSocket）
- 约束：终端宽度有限，需做响应式裁剪；色彩受终端能力限制；无原生滚动，需要虚拟滚动

**GUI（桌面界面）** — 提供两个实现：`a2ui-renderer-egui`（egui 即时模式）与 `a2ui-renderer-iced`（iced 保留模式）
- egui：基于 eframe，每帧根据 A2UI 组件树重建 egui widget；iced：基于 Elm 式的保留模式架构
- 所有 18 个 Basic Catalog 组件均可完整实现（桌面环境无显示限制）
- 系统原生窗口经 eframe / iced 各自的窗口后端（winit）提供
- 适合桌面 Agent 应用（如 Claude Desktop 类产品）

**WebUI（浏览器界面）**

Web 端存在两条**互补而非替代**的渲染路线，分别服务不同象限：**静态派**负责「渲染出内容给人看」，**交互派**负责「完整的交互闭环」。二者共享 `a2ui-core` 的协议类型与 `a2ui-renderer` 的核心语义，只是最终产物与运行环境不同。

| 维度 | 静态派（`a2ui-renderer-web`） | 交互派（TS/React 前端渲染器） |
|------|------------------------------|-------------------------------|
| 形态 | 纯 Rust，服务端把组件树拼成 HTML 字符串 | 浏览器端 TS/React 应用，消费协议消息流后渲染 |
| 产物 | 带 `a2ui-*` class 的静态 HTML | 真实的 React 组件树（可挂 shadcn/MUI/antd 等组件库） |
| 交互 / action 回传 | ❌ 天生不支持（一次性 HTML） | ✅ 事件 → 更新本地 Data Model → `action` 回传服务端 |
| 数据绑定响应性 | ❌ | ✅ Data Model 变更驱动局部重渲染 |
| 运行环境 | 无需 JS 运行时 | 依赖浏览器 + JS 运行时 |
| 典型场景 | SSR / 首屏 / SEO、邮件 HTML、导出 PDF、静态快照、无-JS 预览、TDD 快照参考实现 | 表单填写、按钮点击、选择器等需要用户交互并回传服务端的 Agent UI |

**静态派（`a2ui-renderer-web`）设计要点**
- 输入 A2UI 组件树，输出带 `a2ui-*` 前缀 class 的语义化 HTML 字符串，样式由外部 CSS 决定
- 所有输出经 HTML 转义防范 XSS（见「安全性 → formatString 注入防护」）
- 只做「渲染」这半个闭环，**不处理交互**；点击、输入等事件不产生 `action`
- 价值定位：① 不能跑 JS 的输出目标（邮件 / PDF / 归档快照）；② 纯 Rust、零前端工具链的链路；③ 因输出是纯字符串，天然适合做**快照测试**，作为「协议 → UI 映射」正确性的参考渲染器
- 因交互与响应性缺席，它**不是**交互式 Agent UI 的方案，也不是交互派的雏形——二者是独立的两条线

**交互派（TS/React 前端渲染器，已落地 `clients/web-react`）设计要点**
- 浏览器通过 WebSocket / SSE 收 A2UI 消息流（`createSurface` / `updateComponents` / `updateDataModel` …），在前端维护组件树 + Data Model + 路径解析 + 响应性
- 采用**「协议核心 + 可插拔 ComponentKit」两层**结构：核心层（`src/core` + `src/react`）遍历组件树并只写一次；每个 `ComponentKit`（`src/kits/*`）把 18 个 A2UI 组件类型映射到某套组件库。**已实现两套可热切换的 kit：`shadcn`（shadcn/ui）与 `html`（纯 HTML）**；`<A2UIProvider kit={...}>` 传不同 kit 即切换整套 UI，核心层零改动、Data Model 状态保留（M3 已用测试验证）。可再加 MUI / Ant Design 等。
- ComponentKit 的 props 契约由 **A2UI Catalog schema** 定义（协议是唯一真理来源），而非任何一个组件库；库不支持的能力在 kit 内部优雅降级
- 交互闭环：组件事件 → 立即写回本地 Data Model（响应性传播）→ 触发 `action` 消息经 transport 回传服务端 → 收到 `actionResponse` 写回 `responsePath`
- 传输：`src/transport` 提供 `wsClient`（WebSocket）与 `sseClient`（SSE + `fetch` 回传）两套可互换客户端。服务端侧 `a2ui-transport` **已补 `WebSocketServer`**（`ws_server`，Agent 向浏览器推 `ServerEnvelope`、收 `ClientEnvelope`）；SSE 型 Agent Host 则提供 `GET events`(SSE) + `POST action` 两个端点即可
- 未知 / 自定义组件在 kit 中查不到时渲染占位符，与 `CustomComponentRegistry` 的 miss 逻辑一致

**两派共同点**
- `Image` / `Video` / `AudioPlayer` 在 Web 端有原生 HTML 标签支持，渲染质量最高
- `Icon` 映射为 SVG icon font
- 均适合与 AG-UI 的 Web 前端集成、MCP 客户端

## 跨语言一致性向量（防漂移）

协议核心在本仓库存在**两份实现**：Rust（`a2ui-core` + `a2ui-renderer`，供 TUI/egui/iced/Web 静态派用）与 TS（`clients/web-react/src/core`，供 Web 交互派用）。两份实现若对同一条消息解析出不同结果，UI 就会在不同渲染器间不一致。

为防止这种「漂移」，`tests/conformance/*.json` 存放一批**语言无关**的一致性向量：每个用例是 `{ messages: [envelopes…], expect: { dataModel, resolved, tree } }`，覆盖建树、`updateDataModel` upsert/删除、DynamicValue 解析、`formatString`、ChildList template + `@index`、渐进式占位符等。

- **Rust 侧**：`crates/a2ui-renderer/tests/conformance.rs` 用公开 API ingest 并断言。
- **TS 侧**：`clients/web-react/src/core/*conformance*.test.ts` 用 `SurfaceStore` ingest 并断言。

两侧**同跑同一批 JSON**，任一实现与向量不符即红灯。此机制作为闸门，实际发现并修正过 Rust/TS 之间的 `formatString` 语义、根组件识别、`updateDataModel` 的 `null` 语义等分歧——改协议逻辑时，两份实现必须继续对齐同一份向量。

## 规范扩展登记

本仓库对 basic catalog schema 之外的键/形态的**全部**扩展登记如下（依据 [docs/refactor-step3-renderer-behavior-alignment.md](docs/refactor-step3-renderer-behavior-alignment.md) §3.7）。这些扩展**冻结现状、不再扩散**：新增任何规范外的键或兼容形态前，必须先在此登记并说明理由。

| 扩展 | 涉及方 | 理由与语义 |
|---|---|---|
| CheckBox `checked` 回退键 | 读：四家（经公共 `checkbox_checked`，`value` 优先）；写：`input_writeback` 候选键第二位 | 历史兼容。规范键是 `value`（DynamicBoolean，必填）；`checked` 支持既有「只声明 checked 绑定」的服务端 |
| ChoicePicker 裸字符串 options | 四家（经 `options_decl` 兼容分支） | 历史兼容。规范形态是 `{label, value}` 对象数组；裸字符串按 `label == value` 退化 |
| Image `width` / `height` | egui / iced | 先于规范实现的像素尺寸。规范用 `variant` 尺寸档位（icon/avatar/…）；档位落地时再评估收敛 |
| Modal `title`（`label` 兜底） | web | 规范 Modal 无标题键（`unevaluatedProperties: false`），规范合法输入下该代码永不触发，无害保留 |

## 核心概念详解

### Surface（表面）

一个独立的 UI 区域，拥有独立的组件树和数据模型。生命周期（**已实现：由 `RendererCore` 接入的状态机在每条消息入口强制校验**）：

```
未创建 → createSurface → 活跃（可接收 updateComponents / updateDataModel） → deleteSurface → 已销毁
```

- `surfaceId` 在渲染器生命周期内全局唯一；重复 `createSurface` 被拒绝（`InvalidStateTransition`），删除后同 id 可重新创建（新生命周期）
- 乱序消息（先 update 后 create、删除后 update）被拒绝（`SurfaceNotFound`），不再隐式重建 surface
- `createSurface` 可以只建 surface 不带组件，组件经后续 `updateComponents` 到达（规范标准流程）
- `catalogId` 锁定 Surface 使用的 Catalog，不可更改（要改需删除重建）
- `sendDataModel: true` 时，客户端在每次发送 action 时经信封级 `metadata` 附带该 surface 的完整 data model
- `createSurface` 可以内嵌 `components` 和 `dataModel`，实现单消息 UI 实例化

### Component（组件）

UI 的基本构建块，通过 `updateComponents` 添加或更新。采用**邻接表模型**：组件以扁平列表发送，通过 ID 引用构建树。

- 每个组件有唯一的 `id`（`ComponentId`），必须有一个 `id: "root"` 的组件作为根
- `component` 字段指定组件类型（如 `"Text"`, `"Button"`, `"Column"`），作为 JSON Schema 的 discriminator
- 组件可渐进式到达，客户端应优雅处理缺失引用（渲染占位符，等待后续消息补充）
- 组件属性遵循 A2UI Catalog Schema 严格规则：必须用 `allOf` 组合 `ComponentCommon` 和自身属性

### Data Model（数据模型）

组件绑定的数据，是一个纯 JSON 对象（`serde_json::Value`），通过 `updateDataModel` 进行增删改。

- 路径使用 **JSON Pointer**（RFC 6901）指定
- 支持 upsert 语义：路径存在则更新，不存在则创建，值为 `null` 则删除
- 省略 `path`（或 `/`）则替换整个 data model

### Catalog（组件目录）

声明可用的组件类型和函数定义。一个 Catalog 是 JSON Schema 文件，遵循严格结构：

```
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.com/catalogs/my-v1",
  "catalogId": "https://example.com/catalogs/my-v1",
  "instructions": "Markdown 格式的设计原则...",
  "components": {
    "Text": { /* JSON Schema */ },
    "Button": { /* JSON Schema */ },
    ...
  },
  "functions": {
    "required": { /* JSON Schema with returnType, callableFrom */ },
    "email": { ... },
    ...
  },
  "$defs": {
    "surfaceProperties": { ... },
    "anyComponent": { "oneOf": [...], "discriminator": {"propertyName": "component"} },
    "anyFunction": { "oneOf": [...] }
  }
}
```

**Catalog 严格规则**（v1.0 强制）：
1. 组件和函数定义在顶层 `components` / `functions` map 中
2. `$defs` 中只允许 `surfaceProperties`、`anyComponent`、`anyFunction`
3. 禁止在 `$defs` 中自定义 helper schema
4. 组件 schema 必须用 `allOf` 组合 `ComponentCommon` 引用 + 自身属性
5. 每个组件必须有 `"component": {"const": "组件名"}` 作为 discriminator
6. 函数 schema 必须声明 `returnType` 和 `callableFrom`（默认 `clientOnly`）
7. 顶层只允许固定的一组 key（`$schema`, `$id`, `title`, `description`, `catalogId`, `instructions`, `components`, `functions`, `$defs`）

### Transport（传输层）

协议与传输解耦。传输层必须满足以下契约：

1. **可靠有序投递**：消息按发送顺序到达（状态更新依赖顺序，乱序会损坏 UI 状态）
2. **消息帧分隔**：明确分隔单个 JSON 消息（JSONL 换行 / WebSocket 帧 / SSE 事件）
3. **Metadata 支持**：附加元数据到消息（用于 data model 同步和能力协商）
4. **可选双向通道**：渲染流单向（服务端→客户端），但交互需要返回通道发送 action

支持多种传输绑定：
- **AG-UI**：标准的 Agent-to-User 交互传输
- **A2A**：Agent-to-Agent 映射，标准化 metadata 放置和能力协商
- **MCP**：通过 tool call / tool output / resource subscription 承载 A2UI
- **SSE + JSON RPC**：Web 集成标准方案
- **WebSocket**：双向实时会话
- **REST**：简单场景，不支持流式

### 函数系统（Functions）

服务端通过 `FunctionCall` 引用客户端注册的函数，避免发送可执行代码。

- 函数在 Catalog 的 `functions` map 中声明
- `callableFrom` 控制执行边界：
  - `clientOnly`：只能在客户端执行（如验证、本地计算），服务端调用会被拒绝并返回 `INVALID_FUNCTION_CALL` 错误
  - `remoteOnly`：只能在服务端执行
  - `clientOrRemote`：两端均可执行
- 客户端收到 `callFunction` 时，必须校验函数是否在本地注册且执行边界允许
- 内置函数（Basic Catalog 提供）：`required`、`regex`、`email`、`add`、`concat`、`formatString`、`@index` 等

### 双向绑定与响应性

输入组件（`TextField`、`CheckBox`、`Slider`、`ChoicePicker`、`DateTimeInput`）与 Data Model 建立双向绑定：

1. **读（Model → View）**：组件渲染时从绑定的 `path` 读取值
2. **写（View → Model）**：用户交互时**立即**更新本地 Data Model 对应路径的值
3. **响应性**：本地 Data Model 是单一数据源，一个路径的变更会自动传播到所有绑定到该路径的组件
4. **服务端同步**：本地变更**不**自动发网络请求（被动变更不触发消息）；只有声明式 action 触发时，才在 `sendDataModel` 开启的前提下经信封 `metadata` 附带该 surface 修改后的完整数据

### 作用域与路径解析

- **根作用域**：默认所有组件在根作用域，`/` 开头的路径是绝对路径
- **集合作用域**：当容器组件使用 `ChildList` 的 template 功能时，为数组每个项创建子作用域
  - 不 `/` 开头的路径是相对路径，解析为 `/数组路径/索引/相对路径`
  - `@index` 是系统保留变量，表示当前项的索引
- **混合访问**：子作用域内仍可通过绝对路径访问根作用域数据

## Basic Catalog 参考

A2UI v1.0 定义了标准 Basic Catalog（`catalogs/basic/catalog.json`），包含 18 个组件和 14 个函数。生产环境通常定义自己的 Catalog，但 Basic Catalog 是参考实现和最小公分母。

### 组件清单

| 组件 | 分类 | 核心属性 | 说明 |
|------|------|----------|------|
| `Text` | 显示 | `text` (DynamicString), `variant` (caption/body) | 文本显示，支持简单 Markdown |
| `Image` | 显示 | `url` (DynamicString), `fit`, `variant` (icon/avatar/smallFeature/mediumFeature/largeFeature/header) | 图片展示 |
| `Icon` | 显示 | `name` (枚举 62 个图标名或 `{path}` 动态绑定) | 矢量图标 |
| `Video` | 显示 | `url`, `posterUrl` | 视频播放器 |
| `AudioPlayer` | 显示 | `url`, `description` | 音频播放器 |
| `Row` | 布局 | `children` (ChildList), `justify`, `align` | 水平布局容器 |
| `Column` | 布局 | `children` (ChildList), `justify`, `align` | 垂直布局容器 |
| `List` | 布局 | `children` (ChildList), `direction` (vertical/horizontal), `align` | 列表容器，支持 template 动态生成 |
| `Card` | 容器 | `child` (ComponentId) | 卡片容器，单个子组件 |
| `Tabs` | 容器 | `tabs` (array of {title, child}) | 标签页容器 |
| `Modal` | 容器 | `content` (ComponentId), `trigger` (ComponentId) | 模态对话框 |
| `Divider` | 装饰 | 无特有属性 | 分割线 |
| `Button` | 交互 | `child` (ComponentId), `variant` (default/primary/borderless), `action` (Action), `checks` | 按钮，支持校验自动禁用 |
| `TextField` | 输入 | `value` (DynamicString), `label`, `placeholder`, `variant` (shortText/number/longText/obscured), `checks` | 文本输入框 |
| `CheckBox` | 输入 | `value` (DynamicBoolean), `label` | 复选框 |
| `ChoicePicker` | 输入 | `value` (DynamicStringList), `options`, `variant` (multipleSelection/mutuallyExclusive), `displayStyle` (checkbox/chips), `filterable` | 选择器 |
| `Slider` | 输入 | `value` (DynamicNumber), `min`, `max`, `steps`, `label` | 滑块 |
| `DateTimeInput` | 输入 | `label`, `enableDate`, `enableTime`, `min`, `max` | 日期时间选择器 |

**通用属性**（通过 `ComponentCommon` 混入）：
- `id` (ComponentId, required)
- `accessibility` (AccessibilityAttributes, optional)
- `weight` (number, optional) — 仅在 Row/Column 直接子组件时有效，类似 CSS flex-grow

### 函数清单

| 函数 | 返回类型 | 执行边界 | 参数 | 说明 |
|------|----------|----------|------|------|
| `required` | boolean | clientOnly | `value` | 非空校验 |
| `regex` | boolean | clientOnly | `value`, `pattern` | 正则匹配 |
| `email` | boolean | clientOnly | `value` | 邮箱格式校验 |
| `length` | boolean | clientOnly | `value`, `min`, `max` | 字符串长度约束 |
| `numeric` | boolean | clientOnly | `value`, `min`, `max` | 数值范围约束 |
| `and` | boolean | clientOrRemote | `values` (array of boolean) | 逻辑与 |
| `or` | boolean | clientOrRemote | `values` (array of boolean) | 逻辑或 |
| `not` | boolean | clientOrRemote | `value` (boolean) | 逻辑非 |
| `formatString` | string | clientOrRemote | `value` (template string) | 字符串插值，支持 `${path}` 和 `${functionCall}` |
| `formatNumber` | string | clientOrRemote | `value`, `decimals`, `grouping` | 数字格式化 |
| `formatCurrency` | string | clientOrRemote | `value`, `currency`, `decimals`, `grouping` | 货币格式化 |
| `formatDate` | string | clientOrRemote | `value`, `format` | 日期格式化 |
| `pluralize` | string | clientOrRemote | `value`, `zero`, `one`, `two`, `few`, `many`, `other` | 基于 CLDR 的复数形式 |
| `openUrl` | void | clientOnly | `url` | 打开 URL（浏览器或系统处理器） |

### 系统保留上下文

- **`@index`**：不在 Catalog 的 `functions` 中声明，是系统保留的上下文变量。仅在 `ChildList` template 实例化时可用，返回当前项的整数索引（从 0 开始）。客户端在解析相对路径时自动注入，无需服务端调用。

### surfaceProperties

```json
{
  "iconUrl": "string (URI format)",
  "agentDisplayName": "string"
}
```

两个属性都是可选的，用于标识创建 Surface 的 Agent 或工具。

### Dynamic 类型与 DataBinding / FunctionCall

A2UI 的动态属性不是简单的字符串，而是三种形式的联合：

```rust
// DynamicString 的三种形式
pub enum DynamicValue<T> {
    Literal(T),                                    // "hello"
    Path { path: String },                         // { "path": "/user/name" }
    FunctionCall { call: String, args: Value },   // { "call": "formatString", "args": {...} }
}
```

- `DataBinding`：`{ "path": "/foo/bar" }` — 指向 Data Model 的 JSON Pointer
- `FunctionCall`：`{ "call": "funcName", "args": {...} }` — 调用注册函数
- 字面量：直接使用值

### ChildList 两种模式

```rust
pub enum ChildList {
    // 固定子组件列表
    Array(Vec<ComponentId>),
    // 动态模板：从 Data Model 数组生成子组件
    Object {
        template: ComponentId,  // 用作模板的组件 ID
        path: String,            // Data Model 中数组的路径
    },
}
```

- `Array`：静态子组件，直接引用
- `Object`：动态生成，客户端遍历 `path` 指向的数组，对每项实例化 `template` 组件，进入集合作用域

### Action 两种模式

```rust
pub enum Action {
    // 发送事件到服务端
    Event {
        name: String,
        context: HashMap<String, DynamicValue<Value>>,
        want_response: bool,
        response_path: Option<String>,  // 写入 Data Model 的路径
        action_id: Option<String>,       // want_response 为 true 时必需
    },
    // 执行本地函数
    FunctionCall {
        call: String,
        args: HashMap<String, DynamicValue<Value>>,
    },
}
```

## 消息格式详解

### createSurface

```json
{
  "version": "v1.0",
  "createSurface": {
    "surfaceId": "string (required)",
    "catalogId": "string (required)",
    "surfaceProperties": { "agentDisplayName": "string", "iconUrl": "uri" },
    "sendDataModel": false,
    "components": [ /* Component[] */ ],
    "dataModel": { /* JSON object */ }
  }
}
```

- `surfaceId`：全局唯一标识符
- `catalogId`：建议用你拥有的域名前缀的 URI
- `surfaceProperties`：遵循 Catalog 中 `surfaceProperties` schema 定义
- `sendDataModel`：为 true 时，客户端在每次 action 的 metadata 中附带完整 data model
- `components`：可选，内嵌初始组件树（单消息实例化）
- `dataModel`：可选，内嵌初始数据模型
- 组件列表中必须有一个 `id: "root"` 的组件

### updateComponents

```json
{
  "version": "v1.0",
  "updateComponents": {
    "surfaceId": "string",
    "components": [
      {
        "id": "ComponentId (required)",
        "component": "string (required, 如 'Text')",
        // ... 组件特有属性
      }
    ]
  }
}
```

- 组件以扁平列表提供，关系通过 ID 引用隐式表达（邻接表）
- 组件可以按任意顺序到达，客户端缓冲直到 root 定义完成
- 组件可以引用尚不存在的子组件，客户端应渲染占位符（渐进式渲染）

### updateDataModel

```json
{
  "version": "v1.0",
  "updateDataModel": {
    "surfaceId": "string",
    "path": "JSON Pointer (optional, 默认 '/')",
    "value": "any (optional, 省略则删除 path 对应的 key)"
  }
}
```

- upsert 语义：存在则更新，不存在则创建，值为 null 则删除
- 省略 path 或 path 为 `/` 时替换整个 data model

### deleteSurface

```json
{
  "version": "v1.0",
  "deleteSurface": {
    "surfaceId": "string"
  }
}
```

### actionResponse（v1.0 新增）

```json
{
  "version": "v1.0",
  "actionId": "string (匹配客户端的 actionId)",
  "actionResponse": {
    "value": "any (成功时)",
    "error": { "code": "string", "message": "string" } (失败时)
  }
}
```

### callFunction（v1.0 新增）

```json
{
  "version": "v1.0",
  "functionCallId": "string",
  "wantResponse": false,
  "callFunction": {
    "call": "string (函数名)",
    "args": { /* 函数参数 */ }
  }
}
```

- 客户端必须校验函数的 `callableFrom` 元数据
- `clientOnly` 函数被远程调用时，返回 `INVALID_FUNCTION_CALL` 错误

### 客户端消息

**action**（仅由声明了 `action.event` 的组件交互产生；`surfaceId` / `sourceComponentId` / `timestamp` 必填；`responsePath` 是客户端本地语义，**不上线路**；`sendDataModel` 开启时数据模型经信封级 `metadata` 附带）：
```json
{
  "version": "v1.0",
  "action": {
    "name": "string",
    "surfaceId": "string (必填)",
    "sourceComponentId": "string (必填)",
    "timestamp": "ISO 8601 (必填)",
    "context": { /* 声明的绑定求值后的原生 JSON 值 */ },
    "wantResponse": false,
    "actionId": "string (wantResponse 为 true 时必需)"
  },
  "metadata": { "surfaceId": "string", "dataModel": { /* 该 surface 的完整数据模型 */ } }
}
```

**functionResponse**：
```json
{
  "version": "v1.0",
  "functionResponse": {
    "functionCallId": "string",
    "call": "string",
    "value": "any"
  }
}
```

**error**：
```json
{
  "version": "v1.0",
  "error": {
    "code": "INVALID_FUNCTION_CALL | ...",
    "message": "string",
    "functionCallId": "string (可选)"
  }
}
```

## 关键接口

### Renderer trait（在 `a2ui-renderer` 中定义）

```rust
#[async_trait::async_trait]
pub trait Renderer: Send {
    /// 创建新的 Surface，返回句柄
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle>;
    /// 向指定 Surface 添加或更新组件
    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()>;
    /// 更新指定 Surface 的 Data Model
    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()>;
    /// 销毁指定 Surface
    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()>;
    /// 处理服务端对 action 的响应
    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()>;
    /// 执行服务端请求的客户端函数
    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse>;
    /// 渲染当前帧（各平台自行实现）
    async fn render(&mut self) -> RenderResult<()>;

    /// 处理用户交互，生成客户端信封（sendDataModel 经信封级 metadata 附带，
    /// 裸 ActionMessage 无法承载）。输入类被动变更、无声明 action 的交互
    /// 返回 Ok(None)
    async fn handle_user_event(&mut self, event: UserEvent)
        -> RenderResult<Option<ClientEnvelope>>;
}
```

各平台渲染器的 trait 实现是对 `RendererCore` 对应方法的薄委托（约 10 行/方法）：核心处理消息并返回 `CoreEffects`，平台据此失效渲染缓存（iced 的 RefCell 树/字符串缓存、web 的 `last_html`；TUI/egui 无缓存可忽略）。

### SurfaceState 状态机（已实现：`RendererCore` 在每条消息入口强制校验）

```
┌──────────┐  createSurface   ┌────────┐  deleteSurface   ┌─────────────────────┐
│ 不存在    │ ───────────────▶ │ Active │ ───────────────▶ │ 状态移除（等价不存在）│
└──────────┘                  └────────┘                  └─────────────────────┘
      ▲                                                              │
      └────────────────── 同 id 重新创建 = 新生命周期 ──────────────────┘
```

| 消息 \ 当前状态 | 不存在 | Active |
|---|---|---|
| `createSurface` | ✓ 创建（Pending→Active） | ✗ `InvalidStateTransition` |
| `updateComponents` / `updateDataModel` | ✗ `SurfaceNotFound` | ✓ |
| `deleteSurface` | ✗ `SurfaceNotFound` | ✓（删除后移除全部状态） |

- `Pending` 态在核心内是瞬时的（`create_surface` 单个方法内完成 Pending→Active），不对外暴露中间态
- 删除后不保留墓碑：同 id 可重新创建（新生命周期）；迟到的 update 因 surface 不存在被拒，语义仍受保护

## 数据流时序

```
服务端 (Agent)                         客户端 (Renderer)
     │                                       │
     │  1. createSurface                      │
     │ ─────────────────────────────────────► │
     │                                       │ SurfaceState::Pending → Active
     │                                       │ 构建组件树（可能渐进式）
     │  2. updateComponents                   │
     │ ─────────────────────────────────────► │
     │                                       │ 增量更新组件树
     │  3. updateDataModel                    │
     │ ─────────────────────────────────────► │
     │                                       │ 更新 Data Model，触发响应性重渲染
     │                                       │
     │   用户交互（点击声明了 action.event 的组件） │
     │  ◄──────────────────────────────────── │
     │  handle_user_event → ClientEnvelope     │
     │  （action + 可选 metadata；被动输入只     │
     │    写回本地数据模型，不产生消息）          │
     │                                       │
     │  4. actionResponse (可选)               │
     │ ─────────────────────────────────────► │
     │                                       │ 更新 Data Model / UI
     │                                       │
     │  5. updateComponents / updateDataModel  │
     │ ─────────────────────────────────────► │ 动态更新 UI
     │                                       │
     │  6. deleteSurface                      │
     │ ─────────────────────────────────────► │
     │                                       │ SurfaceState::Deleted
     │                                       │ 清理资源
```

## 关键数据结构的 Rust 表示

### 消息信封

```rust
/// 服务端 → 客户端：信封消息，Exactly one 字段为 Some
#[derive(Debug, Deserialize)]
#[serde(tag = "version", content = "message", rename_all = "camelCase")]
pub enum ServerEnvelope {
    V1_0(V1_0ServerMessage),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum V1_0ServerMessage {
    CreateSurface(CreateSurface),
    UpdateComponents(UpdateComponents),
    UpdateDataModel(UpdateDataModel),
    DeleteSurface(DeleteSurface),
    ActionResponse(ActionResponse),
    CallFunction(CallFunction),
}
```

### 组件邻接表

```rust
/// 组件树：flat list + ID map，延迟构建树
pub struct ComponentForest {
    /// 所有 Surface 的组件存储
    surfaces: HashMap<SurfaceId, ComponentSurface>,
}

pub struct ComponentSurface {
    /// flat list → tree 的构建缓存
    tree: Option<Component>,
    /// 所有组件的 flat map（用于快速查找和更新）
    components: HashMap<ComponentId, Component>,
    /// root 组件 ID
    root: ComponentId,
}
```

### Data Model

```rust
/// Data Model 操作封装，内部用 serde_json::Value
pub struct DataModel {
    value: serde_json::Value,
}

impl DataModel {
    /// JSON Pointer 路径 upsert
    pub fn apply_pointer(&mut self, path: &str, value: Option<serde_json::Value>);
    /// 读取 JSON Pointer 路径的值
    pub fn resolve_pointer(&self, path: &str) -> Option<&serde_json::Value>;
    /// 在集合作用域下解析相对路径
    pub fn resolve_relative(&self, base: &str, relative: &str, index: usize) -> JsonPointer;
}
```

### 路径解析引擎

```rust
/// 路径解析上下文
pub struct ResolveContext<'a> {
    data_model: &'a DataModel,
    scope_stack: Vec<Scope>,
}

pub enum Scope {
    Root,
    Collection { base_path: String, index: usize },
}

impl<'a> ResolveContext<'a> {
    /// 解析 Dynamic* 类型的值（字面量 / 路径 / 函数调用）
    pub fn resolve_dynamic<T>(&self, dynamic: &DynamicValue<T>) -> ResolveResult<T>;
    /// 解析 formatString 中的插值
    pub fn resolve_format_string(&self, template: &str) -> String;
}
```

## Catalog 类型系统映射

详见「Basic Catalog 参考」。核心要点：

- `DynamicValue<T>` 是 A2UI 动态属性的核心抽象，三种形式：`Literal` / `Path` / `FunctionCall`
- `ChildList` 两种模式：`Array`（静态子组件列表） / `Object`（动态模板，从 Data Model 数组生成）
- `Action` 两种模式：`Event`（发送到服务端） / `FunctionCall`（执行本地函数）
- 所有组件必须包含 `ComponentCommon`：`id` (ComponentId) + 可选的 `accessibility`

## 能力协商

### 服务端能力（Server Capabilities）

服务端在握手阶段声明：
- 支持的传输协议
- 支持的消息类型（基础 + 扩展）
- 支持的最大并发 Surface 数
- 是否支持 `sendDataModel`

### 客户端能力（Client Capabilities）

客户端在握手阶段声明：
- 支持的 Catalog 列表（catalog URI → 本地实现）
- 注册的客户端函数（函数名 → 执行边界）
- 是否支持双向 RPC（`actionResponse`、`callFunction`）
- 渲染器特性（如 TUI 的颜色支持、GUI 的原生对话框支持）

## 客户端数据模型同步

当 Surface 的 `sendDataModel: true` 时，客户端在发送 `action` 时经**信封级 `metadata`** 附带该 surface 的完整数据模型（本仓库 WS/JSONL binding 把规范的「transport metadata 机制」定义为信封字段，与 web-react 一致）：

```rust
/// a2ui-core 的信封级 metadata（ClientEnvelope 的可选字段）
pub struct ClientMetadata {
    /// 数据模型所属的 surface
    pub surface_id: String,
    /// 该 surface 的完整数据模型快照
    pub data_model: Option<serde_json::Value>,
}
```

- 附带条件：**事件所属 surface** 的 `sendDataModel == true`（`surface_of` 反查，非「第一个 enabled 的 surface」）
- 数据仅发送给创建该 Surface 的服务端，不泄露给其他 agent；不同 surface 的数据互不越权
- 服务端将收到的 data model 视为 action 触发时的完整客户端状态

## 标准校验错误格式

```rust
pub struct ValidationError {
    pub message: String,
    pub component_id: ComponentId,
    pub check_index: usize,
}
```

- 输入组件的 `checks` 列表中的每个校验失败都产生一个 `ValidationError`
- 按钮的 `checks` 失败时自动禁用（`enabled: false`）
- 条件校验（`and` / `or`）递归求值

## 开发路线

### 第一阶段：核心协议

1. 搭建 workspace 结构，初始化 `a2ui-core`
2. 实现协议类型定义（按 three JSON schemas 映射 Rust 类型）
3. 实现消息反序列化（envelope 分发 + 各消息类型）
4. 实现 Surface 状态机
5. 实现 JSON Schema 校验（Catalog 合规性检查）
6. 为 `a2ui-core` 编写完整的单元测试和文档测试

### 第二阶段：渲染器骨架

7. 实现 `a2ui-renderer` 的 `Renderer` trait 和组件树管理
8. 实现 Data Model 存储和 JSON Pointer 路径解析
9. 实现作用域系统（根作用域 + 集合作用域 + `@index`）
10. 实现 `formatString` 和基础函数的调度
11. 实现双向绑定的响应性传播

### 第三阶段：TUI 渲染器落地

12. 实现 `a2ui-renderer-tui`：ratatui widget 映射
13. 实现键盘事件 → action 消息转换
14. 实现焦点管理和本地函数注册
15. 端到端集成测试：用 JSONL 示例流验证完整渲染流程

### 第四阶段：传输层与扩展

16. 实现 `a2ui-transport` 抽象 trait
17. 实现至少一种传输绑定（建议 AG-UI 或 WebSocket）
18. 实现能力协商握手
19. 扩展其他渲染器（GUI / Web）

## 错误处理

```rust
/// 渲染器错误分类
#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    /// Surface 不存在（ID 无效或已销毁）
    #[error("Surface not found: {0}")]
    SurfaceNotFound(SurfaceId),

    /// Surface ID 冲突（createSurface 时 ID 已存在）
    #[error("Surface ID already exists: {0}")]
    SurfaceIdConflict(SurfaceId),

    /// 组件 ID 无效（不符合 UAX #31）
    #[error("Invalid component ID: {0}")]
    InvalidComponentId(String),

    /// 组件引用不存在（children 引用了未定义的组件）
    #[error("Component reference not found: {0}")]
    ComponentNotFound(ComponentId),

    /// Catalog 未加载或 ID 不匹配
    #[error("Catalog not found: {0}")]
    CatalogNotFound(CatalogId),

    /// 函数未注册或执行边界不允许
    #[error("Function not available: {0}")]
    FunctionNotAvailable(String),

    /// 状态机违规（在错误状态执行了非法操作）
    #[error("Invalid state transition: current={current:?}, attempted={attempted:?}")]
    InvalidStateTransition {
        current: SurfaceState,
        attempted: StateOperation,
    },

    /// JSON 反序列化失败
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),

    /// 传输层错误
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
}
```

错误传播原则：
- 解析错误（`Deserialization`）应立即终止消息流处理，报告给传输层
- 语义错误（`SurfaceNotFound`、`ComponentNotFound`）应记录日志并跳过该消息，不终止流
- 状态机违规（`InvalidStateTransition`）是严重错误，应终止 Surface 并上报
- 客户端函数执行错误应包装为 `error` 消息返回服务端

## 渐进式渲染与占位符

当组件以任意顺序渐进到达时，渲染器必须优雅处理缺失引用：

```
收到 updateComponents: [root, title, button_label, button]
                            │
                            ▼
                  root 定义 children: [title, button]
                  title 已到达 → 渲染 Text
                  button 引用 button_label（已到达） → 渲染 Button
```

- 已到达且引用完整的组件立即渲染
- 引用缺失的组件渲染**占位符**（平台特定：TUI 用空格/虚线，GUI 用灰色框，Web 用 skeleton）
- 占位符尺寸由组件自身声明或 catalog 默认值决定
- 当缺失的组件后续到达时，占位符替换为真实内容
- `root` 组件未到达前，所有 updateComponents 消息被缓冲，不渲染

## 重渲染策略

采用**声明式响应性**模型，而非命令式重渲染：

- Data Model 更新时，不是"通知所有组件刷新"，而是**标记路径变更**
- 组件在渲染时声明自己的依赖路径集合
- 只有依赖了变更路径的组件才参与重渲染
- 实现方式：每个 `DynamicValue` 绑定在解析时注册其路径到 `ComponentDependency` 集合
- DataModel::apply_pointer 触发时，反向查找受影响组件，仅重建这些组件

```
DataModel 更新 /user/name → "Alice"
        │
        ▼
DependencyGraph 查询哪些组件依赖 /user/name
        │
        ├── Text(id="name_label", text={path:"/user/name"}) → 需要重渲染
        ├── Text(id="title", text={path:"/title"})           → 不需要
        └── Button(id="submit", checks=[...path:/user/name])  → 需要（checks 依赖）
```

## 版本兼容

- 信封消息的 `version` 字段标识协议版本（如 `"v1.0"`）
- 版本为字符串而非枚举，支持未来新增版本而不破坏旧代码
- 渲染器启动时声明自己支持的最高版本
- 收到不支持的版本时，拒绝创建 Surface 并返回错误
- v1.0 的消息类型在 v2.0 中应保持向后兼容（旧类型不删除，标记为 deprecated）

## 响应式 Data Model 写回（responsePath）

当客户端发送 `action` 并设置 `wantResponse: true` 且 `responsePath: "/some/path"` 时：

```rust
// 收到 actionResponse 后：
// 1. 如果 responsePath 存在，将 response.value 写入该路径
// 2. 触发该路径依赖的组件重渲染
// 3. 如果 response.error 存在，将错误信息写入 path 或显示 toast
```

这是服务端异步计算结果回写给客户端 Data Model 的标准机制。

## 内置函数详解

### formatString

字符串模板插值与类型转换函数。

**语法**：`formatString(template, bindings...)`

```json
{
  "call": "formatString",
  "args": {
    "template": "Hello, {name}! You have {count} messages.",
    "bindings": {
      "name": { "path": "/user/name" },
      "count": { "path": "/user/messageCount" }
    }
  }
}
```

- `{name}` 被替换为 `/user/name` 的值，自动类型转换
- 支持嵌套插值：`{greeting}` 其中 `greeting` 本身是另一个 formatString
- 客户端和服务端均可执行（`callableFrom: clientOrRemote`）

### @index

集合迭代中的当前索引。

- **作用域限制**：仅在 `ChildList` 的 template 实例化上下文中可用
- **参数**：无
- **返回**：当前项的整数索引（从 0 开始）
- **使用场景**：在 template 的组件中显示序号、交替样式等

```json
{
  "id": "item_index",
  "component": "Text",
  "text": { "call": "@index" }
  // 在 list template 中渲染为 "0", "1", "2", ...
}
```

### prompt-generate-validate 循环

服务端生成 UI → 客户端校验（通过 checks）→ 校验失败时服务端收到错误并修正 → 重新生成。这是 A2UI 的核心交互模式。

标准校验错误格式：
```rust
pub struct CheckError {
    pub message: String,       // 面向用户的错误描述
    pub component_id: ComponentId,  // 出错的组件
    pub check_index: usize,    // checks 列表中的索引
}
```

## 可观测性

流式协议的调试需要结构化日志和 trace：

- 每条消息进入渲染器时记录 `trace` 级别日志：`{surface_id, message_type, timestamp}`
- 组件树变更时记录 `debug` 级别：添加/更新/删除了哪些组件
- Data Model 变更时记录 `debug` 级别：路径、旧值、新值
- 错误时记录 `error` 级别：完整错误上下文
- 使用 `tracing` crate，支持 structured logging 和 OpenTelemetry 导出
- transport 层记录网络级事件（连接/断开/重连），与渲染层日志分离

## 自定义组件扩展

客户端可以注册超出 Basic Catalog 的自定义组件渲染器：

```rust
pub struct CustomComponentRegistry {
    /// 组件类型名 → 渲染函数
    renderers: HashMap<String, BoxedComponentRenderer>,
}

pub trait ComponentRenderer: Send + Sync {
    fn render(
        &self,
        ctx: &mut RenderContext,
        component: &Component,
        children: &[RenderedChild],
    ) -> Result<RenderedNode, RendererError>;
}
```

- 注册表在 `Renderer` trait 实现时初始化
- 遇到未知组件类型时，尝试在注册表中查找
- 未找到时渲染为"不支持的组件"占位符（包含组件类型名）
- 自定义组件不影响协议层——协议只传递组件类型字符串，渲染是客户端自由

## Surface 资源管理

- 渲染器应设置最大并发 Surface 数量上限（建议默认 100）
- 超出上限时，`createSurface` 返回 `SurfaceIdConflict` 或拒绝创建
- `deleteSurface` 时完整清理：组件树、Data Model、依赖图、占位符
- 长时间无活动的 Surface 可考虑自动销毁（需配置，协议未强制）
- 内存敏感场景（如 TUI）应监控 Surface 数量，必要时驱逐最久未使用的

## 安全性

### 协议层面的安全机制

**1. 函数执行边界 enforcement**

A2UI v1.0 最核心的安全机制。客户端必须在运行时严格校验 `callableFrom`：

| 边界 | 含义 | 客户端行为 |
|------|------|------------|
| `clientOnly` | 只能在客户端执行（如验证、本地计算） | 收到服务端 `callFunction` 时拒绝，返回 `INVALID_FUNCTION_CALL` |
| `remoteOnly` | 只能在服务端执行 | 客户端不应在本地注册该函数 |
| `clientOrRemote` | 两端均可执行 | 正常执行 |

未在本地注册的函数收到 `callFunction` 时也必须拒绝（无论 `callableFrom` 为何值）。

**2. Catalog 信任链**

- 客户端只执行其**已加载 Catalog** 中声明的组件和函数
- 收到未知组件类型时渲染占位符，不执行任何逻辑
- `createSurface` 的 `catalogId` 必须与客户端已加载的 Catalog URI 匹配
- 支持内联 Catalog（`inlineCatalogs`），但客户端应校验其结构合规性

**3. Data Model 隔离**

- `sendDataModel` 的数据**仅发送给创建该 Surface 的服务端**，通过 transport metadata 的 `surfaceId` 标签实现 targeting
- 不同 Surface 的 Data Model 完全隔离，无共享路径
- 服务端收到的 Data Model 应视为不可信输入，需做 schema 校验

**4. 标识符注入防护**

- 所有 `ComponentId`、函数名、组件类型名必须通过 Unicode UAX #31 校验（正则 `^[\p{XID_Start}_][\p{XID_Continue}]*$`）
- 禁止 whitespace 和 `Pattern_Syntax` 字符，防止通过 crafted ID 进行注入攻击
- `@` 命名空间保留给系统，客户端应拒绝服务端发送的 `@` 前缀自定义标识符

**5. 能力协商最小权限**

- 客户端只声明自己**实际支持**的 Catalog 和函数
- 不声明不支持的功能，避免服务端尝试调用
- `acceptsInlineCatalogs` 默认为 `false`，防止服务端注入恶意 Catalog

### 传输层安全

- A2UI 协议本身不加密，**依赖传输层提供 TLS**（WebSocket over TLS / HTTPS SSE）
- 能力协商（Capabilities）应在 TLS 隧道内完成，防止信息泄露
- 建议在 transport 层实现证书校验和 hostname 验证

### 实现层面的安全考虑

**6. formatString 注入防护**

`formatString` 的 `${expression}` 插值如果直接拼接到渲染目标（如 HTML），可能导致 XSS：

```rust
// 危险：直接拼接
let html = format!("<p>{}</p>", resolved_value);

// 安全：转义后拼接
let html = format!("<p>{}</p>", html_escape(resolved_value));
```

- 所有渲染器必须对 `formatString` 的解析结果做**上下文相关的转义**
- Web 渲染器：HTML escape；TUI 渲染器：ANSI escape code 防护；GUI 渲染器：平台原生安全
- 嵌套插值需递归转义

**7. JSON Pointer 路径遍历防护**

```rust
// 防止路径逃出 Data Model 边界
pub fn resolve_pointer_safe(&self, path: &str) -> Result<&Value, DataModelError> {
    // 1. 解析 JSON Pointer
    // 2. 检查解析后的路径是否在 Data Model 的根范围内
    // 3. 禁止通过 .. 或空路径段逃逸
}
```

- 服务端发送的 `path` 可能包含恶意构造的 JSON Pointer（如 `//` 或 `/%00`）
- 客户端应校验路径解析结果是否仍在 Data Model 根对象范围内
- 禁止路径遍历到 Data Model 外部

**8. 拒绝服务（DoS）防护**

| 攻击面 | 防护措施 |
|--------|----------|
| 无限 Surface 创建 | 最大并发 Surface 数限制（默认 100） |
| 巨型组件树 | 单 Surface 最大组件数限制（建议 1000） |
| 巨型 Data Model | 单 Surface Data Model 大小限制（建议 1MB） |
| Deeply nested children | 组件树深度限制（建议 50 层） |
| formatString 递归 | 插值深度限制（建议 10 层），防止无限递归 |
| 高频 updateDataModel | 消息速率限制（transport 层实现） |
| Catalog 解析 | JSON Schema 解析超时和内存限制 |

**9. 序列化安全**

- `serde_json` 反序列化时启用 `deny_unknown_fields` 对所有协议消息类型
- Catalog JSON 反序列化时严格校验结构（只允许规定的顶层 key）
- 禁止 `serde_json::Value` 直接参与格式化输出（必须先转义）

**10. 错误信息不泄露敏感数据**

- 错误消息中不包含内部路径、内存地址、原始 JSON payload
- `INVALID_FUNCTION_CALL` 错误只返回函数名，不返回服务器参数
- 开发/调试模式可开启详细错误，生产模式必须精简

### 安全 Checklist（实现时必须验证）

- [ ] `callFunction` 的 `callableFrom` 在客户端严格 enforcement
- [ ] 未注册的函数调用被拒绝
- [ ] `ComponentId` 通过 UAX #31 校验
- [ ] `formatString` 结果在渲染前做上下文转义
- [ ] JSON Pointer 路径解析有边界检查
- [ ] Surface 数量、组件树大小有上限
- [ ] `sendDataModel` 数据只发送给目标服务端
- [ ] Catalog 结构合规性校验（禁止自定义 `$defs`）
- [ ] 反序列化使用 `deny_unknown_fields`
- [ ] 生产模式错误信息不泄露敏感数据
