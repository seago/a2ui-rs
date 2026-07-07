# a2ui-rs

<div align="center">

**A2UI（Agent-to-UI）Protocol v1.0 的 Rust 实现**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-725%20passed-brightgreen.svg)](https://github.com/seago/a2ui-rs)

</div>

## 目录

- [项目简介](#项目简介)
- [架构总览](#架构总览)
- [Workspace Crate 说明](#workspace-crate-说明)
- [交互式 Web 渲染器（B2）](#交互式-web-渲染器b2)
- [快速开始](#快速开始)
- [运行示例](#运行示例)
- [协议支持](#协议支持)
- [功能特性](#功能特性)
- [防漂移一致性向量](#防漂移一致性向量)
- [开发指南](#开发指南)
- [测试](#测试)
- [项目结构](#项目结构)
- [协议规范](#协议规范)
- [许可证](#许可证)

## 项目简介

`a2ui-rs` 是 [A2UI（Agent to UI）Protocol v1.0](https://a2ui.org/specification/v1.0-a2ui/) 的 Rust 实现。A2UI 是一个基于 JSON 的**流式 UI 协议**：AI Agent（服务端）通过 JSON 消息流实时描述和更新 UI 界面，Renderer（客户端）逐条解析并逐步渲染，并把用户交互结构化回传。协议的核心哲学是 **UI 结构与数据分离**，支持渐进式渲染——客户端每收到一条消息就立即增量构建 UI，无需等待完整 payload。

### 核心能力

- 📡 **流式协议解析**：逐条消息增量构建 UI，支持 JSONL / WebSocket / SSE 传输
- 🎨 **多端渲染**：终端（TUI）、桌面（egui / iced）、Web（静态 HTML + 交互式 React）
- 🧩 **可插拔 ComponentKit**：交互式 Web 渲染器采用「协议核心 + 可切换组件库」架构，内置 shadcn/ui 与纯 HTML 两套 kit，换 kit 即换整套设计体系
- 🔗 **双向数据绑定**：Data Model 与 UI 组件自动同步，响应式传播变更
- 🧱 **18 个标准组件 + 14 个函数**：完整实现 Basic Catalog
- 🔒 **安全执行边界**：严格的 `callableFrom` 函数执行边界 enforcement
- 🧠 **增量依赖追踪**：`DependencyGraph` 精确追踪依赖，按需重渲染
- 🛡️ **防漂移**：Rust 与 TS 两份协议实现同跑一套一致性向量，保证语义不走样

## 架构总览

协议与渲染实现完全分离。传输层将 JSON 消息流送入协议核心，核心层维护 Surface 状态并驱动各渲染器；用户交互经协议回传 Agent。

```
┌──────────┐   A2UI JSON 流    ┌──────────────────┐   ┌────────────────────┐
│  Agent    │─────────────────▶│  Transport 传输层  │──▶│  Protocol Core 核心 │
│（服务端） │  createSurface…   │  WS / SSE / JSONL │   │ 消息解析·状态机·校验 │
└──────────┘◀─────────────────└──────────────────┘   └─────────┬──────────┘
              action 回传                                        │
        ┌──────────────┬──────────────┬──────────────┬──────────┴──────────┐
        ▼              ▼              ▼              ▼                     ▼
  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────────┐   ┌──────────────────┐
  │ TUI       │   │ egui GUI  │   │ iced GUI  │   │ Web 静态派   │   │ Web 交互派（B2）  │
  │（ratatui）│   │（原生窗口）│  │（原生窗口）│   │ HTML 字符串  │   │ TS/React +        │
  └──────────┘   └──────────┘   └──────────┘   └─────────────┘   │ 可插拔 ComponentKit│
                                                                 │ shadcn / html …   │
   └──── Rust 渲染器：共用 a2ui-renderer 核心 + a2ui-transport ────┘   └──────────────────┘
                                                                    浏览器端独立 TS 实现
```

- **Rust 系渲染器**（TUI / egui / iced / Web 静态派）采用「**RendererCore + 平台 widget 映射**」结构：协议状态与消息处理流水线（含 Surface 生命周期状态机）收敛在 `a2ui-renderer` 的 `RendererCore`，各平台渲染器组合核心、只保留平台特有的 widget 映射与渲染缓存，共用 `a2ui-transport` 传输。
- **Web 交互派（B2）** 是浏览器端独立的 TS/React 实现（`clients/web-react`），靠 `a2ui-core` ↔ 前端 `src/contracts` 的**同一份协议类型** + [一致性向量](#防漂移一致性向量) 与 Rust 侧对齐。
- **静态派 vs 交互派**：`a2ui-renderer-web`（Rust）输出一次性 HTML 字符串，适合 SSR / 无-JS 场景；B2 支持完整交互、双向绑定与 action 回传。详见 [ARCHITECTURE.md](ARCHITECTURE.md)。

## Workspace Crate 说明

| Crate | 职责 | 关键依赖 |
|-------|------|----------|
| [`a2ui-core`](crates/a2ui-core/) | 协议类型定义、消息枚举、JSON Schema 校验、Surface 状态机 | `serde`, `serde_json`, `thiserror` |
| [`a2ui-renderer`](crates/a2ui-renderer/) | `Renderer` trait、`RendererCore`（共享协议状态 + 消息流水线 + 生命周期状态机）、组件森林、数据绑定、路径解析、函数调度、DependencyGraph、SurfaceLru、CustomComponentRegistry | `a2ui-core` |
| [`a2ui-renderer-tui`](crates/a2ui-renderer-tui/) | 终端渲染器 | `ratatui`, `crossterm`, `a2ui-renderer` |
| [`a2ui-renderer-egui`](crates/a2ui-renderer-egui/) | 桌面渲染器（egui + eframe 即时模式 GUI） | `egui`, `eframe`, `a2ui-renderer` |
| [`a2ui-renderer-iced`](crates/a2ui-renderer-iced/) | 桌面渲染器（iced 保留模式 GUI） | `iced`, `a2ui-renderer` |
| [`a2ui-renderer-web`](crates/a2ui-renderer-web/) | Web **静态派**：服务端把组件树渲染为 HTML 字符串，XSS 安全转义 | `a2ui-renderer` |
| [`a2ui-transport`](crates/a2ui-transport/) | 传输层抽象 + JSONL / WebSocket 客户端 / WebSocket 服务端 + 能力协商 | `tokio`, `tokio-tungstenite`, `a2ui-core` |
| [`a2ui-cli`](crates/a2ui-cli/) | 命令行入口，集成 transport + renderer | `clap`, `a2ui-transport` |

> Web **交互派**（B2）不是 Rust crate，而是独立的 TS/React 项目 [`clients/web-react`](clients/web-react/)。

**架构约束：**

- `a2ui-transport` 只负责消息收发和会话管理，不含渲染逻辑
- `a2ui-renderer` 定义 `Renderer` trait，各平台渲染器实现该 trait
- `a2ui-core` 是唯一直接依赖 `serde_json` 的 crate，下游通过 Rust 类型交互
- Surface 生命周期严格由状态机管理：`createSurface` → `Active` → `deleteSurface`

## 交互式 Web 渲染器（B2）

[`clients/web-react`](clients/web-react/) 是浏览器端的**交互式** Web 渲染器（TS + React + Vite），实现「协议核心 + 可插拔 ComponentKit」两层架构：

```
WS/SSE ─▶ ① 协议核心（纯 TS）  ─▶ ② 渲染核心（React，库无关） ─▶ ③ ComponentKit
          src/core                src/react                      src/kits/*
          SurfaceStore /           A2UIProvider / Surface /       shadcn / html …
          DataModel / 响应性        tree-walker
```

- **`src/contracts`** — 冻结的接口缝：协议类型、`SurfaceStore`、`ComponentKit` props。
- **`src/core`** — 纯 TS 协议引擎：消息解析、JSON Pointer DataModel、组件森林、作用域路径解析（含 `@index`）、依赖图响应性、`formatString`、函数调度、交互回传。
- **`src/react`** — 库无关渲染核心：`A2UIProvider` / `Surface` / tree-walker。
- **`src/kits`** — 可插拔组件库：`shadcn/`（shadcn/ui）、`html/`（纯 HTML）。换 kit 即换整套 UI，核心零改动、状态保留。
- **`src/transport`** — 浏览器传输客户端：`wsClient`（WebSocket）与 `sseClient`（SSE + `fetch` 回传），可互换。

```bash
cd clients/web-react
npm install
npm run demo    # 打开可插拔 kit 示例页（实时切换 shadcn / html）
npm test        # 155 个前端测试
```

## 快速开始

### 环境要求

- Rust 1.75+
- Node.js 18+（仅交互式 Web 渲染器 B2 需要）
- 现代终端（TUI 需 ANSI 支持）、桌面环境（egui / iced 需系统窗口）

### 安装

```bash
git clone git@github.com:seago/a2ui-rs.git
cd a2ui-rs
```

### 编译

```bash
cargo build --workspace          # 编译整个 workspace
cargo build -p a2ui-core         # 编译指定 crate
```

### 运行测试

```bash
cargo test --workspace           # 全部 Rust 测试
cargo test -p a2ui-renderer      # 指定 crate
cargo fmt && cargo clippy --workspace
```

### 使用 CLI

从 STDIN 读取 JSONL 流并渲染到终端：

```bash
echo '{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic","sendDataModel":false}}' \
  | cargo run --bin a2ui -- render
```

## 运行示例

**Rust 渲染器：**

```bash
# TUI 终端渲染器（交互式，q/Esc 退出）
cargo run --example simple_tui  -p a2ui-renderer-tui

# egui 桌面渲染器（原生窗口，18 组件）
cargo run --example simple_gui  -p a2ui-renderer-egui
# 另有 login / contact_card / restaurant 示例

# iced 桌面渲染器
cargo run --example simple_iced -p a2ui-renderer-iced
# 另有 login_iced / restaurant_iced 示例

# Web 静态派（生成 HTML 文件）
cargo run --example simple_web  -p a2ui-renderer-web

# WebSocket 演示服务端（配合 B2 前端端到端联调）
cargo run --example serve_demo  -p a2ui-transport   # 监听 ws://127.0.0.1:8765
```

**交互式 Web 渲染器（B2）：**

```bash
cd clients/web-react
npm install
npm run demo    # 可插拔 kit 示例页（无需服务端）
npm run dev     # App 连 serve_demo 的 ws://127.0.0.1:8765
```

## 协议支持

### 消息类型

| 消息 | 方向 | 说明 | 状态 |
|------|------|------|:----:|
| `createSurface` | Server → Client | 创建 UI Surface，可内嵌组件树和 data model | ✅ |
| `updateComponents` | Server → Client | 邻接表式增量更新组件 | ✅ |
| `updateDataModel` | Server → Client | JSON Pointer 路径 upsert（`null` 删除，符合 v1.0） | ✅ |
| `deleteSurface` | Server → Client | 销毁 Surface 并清理资源 | ✅ |
| `actionResponse` | Server → Client | 服务端响应客户端 action | ✅ |
| `callFunction` | Server → Client | 服务端调用客户端注册函数 | ✅ |
| `action` | Client → Server | 声明式 server action（组件声明 `action.event` 才发送；`sendDataModel` 经信封 metadata 附带数据） | ✅ |
| `functionResponse` | Client → Server | 客户端函数执行结果 | ✅ |
| `error` | Client → Server | 客户端错误上报 | ✅ |

### Basic Catalog 组件（18 个）

| 分类 | 组件 | 说明 |
|------|------|------|
| 显示 | `Text` `Image` `Icon` `Video` `AudioPlayer` | 文本、图片、图标、视频、音频 |
| 布局 | `Row` `Column` `List` | 水平/垂直布局、列表（支持动态模板） |
| 容器 | `Card` `Tabs` `Modal` | 卡片、标签页、模态对话框 |
| 输入 | `TextField` `CheckBox` `ChoicePicker` `Slider` `DateTimeInput` | 文本输入、复选框、选择器、滑块、日期时间 |
| 交互 | `Button` | 按钮（支持 action 和校验禁用） |
| 装饰 | `Divider` | 分割线 |

### 函数（14 个）

| 类别 | 函数 | 执行边界 |
|------|------|----------|
| 校验 | `required` `regex` `email` `length` `numeric` | `clientOnly` |
| 逻辑 | `and` `or` `not` | `clientOrRemote` |
| 格式化 | `formatString` `formatNumber` `formatCurrency` `formatDate` `pluralize` | `clientOrRemote` |
| 系统 | `openUrl` | `clientOnly` |

## 功能特性

### 渐进式渲染

组件以扁平邻接表按任意顺序到达，渲染器自动缓冲并渐进构建。引用完整的组件立即渲染，引用缺失的渲染为平台特定占位符（TUI 虚线框 / GUI 灰框 / Web skeleton），后续到达自动替换。

### 双向数据绑定

输入组件（TextField、CheckBox、Slider 等）与 Data Model 双向绑定：

1. **读（Model → View）**：渲染时从绑定的 JSON Pointer 读取值
2. **写（View → Model）**：用户交互时立即更新本地 Data Model
3. **响应性**：路径变更自动传播到所有绑定该路径的组件

### DependencyGraph 增量重渲染

声明式依赖追踪而非全量刷新：Data Model 更新时精确定位受影响组件，只重渲染依赖了变更路径的部分。

### Surface 生命周期管理

状态机驱动 `未创建 → createSurface → Active → deleteSurface → Deleted`；`SurfaceLru` 自动驱逐（计数上限 + 空闲超时）；删除时完整清理组件树 / Data Model / 依赖图。

### 可插拔 ComponentKit（B2）

交互式 Web 渲染器把「协议核心」与「组件库」解耦：`<A2UIProvider kit={...}>` 传不同 kit 即切换整套 UI（内置 shadcn / html 两套），核心与渲染层零改动，Data Model 状态跨切换保留。

### 安全机制

- **函数执行边界 enforcement**：严格校验 `callableFrom`，未注册函数调用被拒绝
- **标识符校验**：`ComponentId` 通过 Unicode UAX #31 校验
- **XSS 防护**：Web 渲染器对输出做上下文转义
- **JSON Pointer 边界检查**：防止路径逃出 Data Model 根范围
- **DoS 防护**：Surface 数量上限、组件树大小、递归深度限制

## 防漂移一致性向量

`tests/conformance/*.json` 是一批**语言无关**的协议一致性向量（建树、`updateDataModel` upsert/删除、DynamicValue、`formatString`、ChildList template + `@index`、渐进式占位符）。它们由 **Rust 侧**（`crates/a2ui-renderer/tests/conformance.rs`）与 **TS 侧**（`clients/web-react/src/core/*conformance*.test.ts`）**同跑同一批用例**，作为闸门确保两份协议实现语义一致。此机制曾实际发现并修正了 Rust/TS 之间的 `formatString`、根组件识别、`updateDataModel` null 语义等多处分歧。

## 开发指南

### TDD 开发模式

项目严格遵循 **红 → 绿 → 重构** 的 TDD 循环：先写失败测试 → 最少代码转绿 → 保持绿灯重构。

### 测试分层

- **单元测试**（`#[cfg(test)]`）：源码文件末尾，测单个函数/模块
- **集成测试**（`tests/`）：从 crate 外调用公开 API
- **文档测试**（`///`）：确保文档示例可编译运行
- **前端**（Vitest + React Testing Library）：`clients/web-react`

### 代码风格

- 错误处理用 `thiserror`；异步接口统一 `async fn`
- 公共 API 必须有文档注释（`///`）；序列化统一 `serde` + `serde_json`

## 测试

| 层 | 测试数 |
|----|:------:|
| Rust workspace（单元 + 集成 + E2E + 文档测试） | **668** |
| 交互式 Web 渲染器 B2（Vitest / RTL） | **155** |
| **合计** | **823** |

```bash
cargo test --workspace                       # Rust
cd clients/web-react && npm test             # 前端
```

## 项目结构

```
a2ui-rs/
├── ARCHITECTURE.md                 # 架构设计文档（静态派/交互派分工等）
├── CLAUDE.md                       # 协作指引
├── Cargo.toml                      # Workspace 配置
├── crates/
│   ├── a2ui-core/                  # 协议核心：message / component / datamodel / schema / state
│   ├── a2ui-renderer/              # 渲染核心：renderer.rs / renderer_core.rs / component_forest.rs
│   │                               #   data_binding.rs / dependency_graph.rs / path_resolver.rs
│   │                               #   function_dispatcher.rs / format_string.rs / surface_lru.rs …
│   ├── a2ui-renderer-tui/          # TUI（ratatui）+ examples/simple_tui.rs
│   ├── a2ui-renderer-egui/         # egui 桌面 + examples/{simple_gui,login,contact_card,restaurant}.rs
│   ├── a2ui-renderer-iced/         # iced 桌面 + examples/{simple_iced,login_iced,restaurant_iced}.rs
│   ├── a2ui-renderer-web/          # Web 静态派（HTML 字符串）+ examples/simple_web.rs
│   ├── a2ui-transport/             # transport.rs / websocket.rs / ws_server.rs / jsonl.rs
│   │                               #   + examples/serve_demo.rs
│   └── a2ui-cli/                   # 命令行工具
├── clients/
│   └── web-react/                  # 交互式 Web 渲染器（B2，TS/React）
│       └── src/
│           ├── contracts/          # 协议类型 / SurfaceStore / ComponentKit 契约
│           ├── core/               # 纯 TS 协议引擎
│           ├── react/              # 渲染核心（Provider / Surface / tree-walker）
│           ├── kits/{shadcn,html}/ # 可插拔组件库
│           ├── transport/          # wsClient / sseClient
│           └── demo/               # 可插拔 kit 示例页
├── tests/
│   └── conformance/                # Rust/TS 共享的协议一致性向量
└── docs/
```

## 协议规范

- **协议版本**：A2UI Protocol v1.0
- **规范地址**：[a2ui.org/specification/v1.0-a2ui](https://a2ui.org/specification/v1.0-a2ui/)
- **架构设计**：[ARCHITECTURE.md](ARCHITECTURE.md)

v1.0 相比 v0.9 的主要变化：

- 双向 RPC：服务端可调用客户端函数（`callFunction` / `functionResponse`）
- 单消息 UI 实例化：`createSurface` 可内嵌完整组件树和 data model
- 解耦品牌化：用可扩展的 `surfaceProperties` 替代硬编码主题属性
- 增强的 Catalog Schema：函数定义改为对象映射（O(1) 查找）
- 严格标识符规范：Unicode UAX #31 命名规则

## 许可证

[MIT](LICENSE)

---

<div align="center">
  <sub>Built with 🦀 Rust + ⚛️ React · A2UI Protocol v1.0 · 2026</sub>
</div>
