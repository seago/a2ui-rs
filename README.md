# a2ui-rs

<div align="center">

**A2UI（Agent-to-UI）Protocol v1.0 的 Rust 实现**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-394%20passed-brightgreen.svg)](https://github.com/yufeng108/a2ui-rs)

</div>

## 目录

- [项目简介](#项目简介)
- [架构总览](#架构总览)
- [Workspace Crate 说明](#workspace-crate-说明)
- [快速开始](#快速开始)
- [运行示例](#运行示例)
- [协议支持](#协议支持)
- [功能特性](#功能特性)
- [开发指南](#开发指南)
- [测试](#测试)
- [项目结构](#项目结构)
- [协议规范](#协议规范)
- [许可证](#许可证)

## 项目简介

`a2ui-rs` 是 [A2UI（Agent to UI）Protocol v1.0](https://a2ui.org/specification/v1.0-a2ui/) 的完整 Rust 实现。A2UI 是一个基于 JSON 的**流式 UI 协议**：AI Agent（服务端）通过 JSON 消息流实时描述和更新 UI 界面，Renderer（客户端）逐条解析并逐步渲染。协议的核心哲学是 **UI 结构与数据分离**，支持渐进式渲染——客户端每收到一条消息就立即增量构建 UI，无需等待完整 payload。

### 核心能力

- 📡 **流式协议解析**：逐条消息增量构建 UI，支持 JSONL / WebSocket / SSE 等传输方式
- 🎨 **多平台渲染**：终端（TUI）、桌面窗口（GUI）、浏览器（Web）三大渲染器齐备
- 🔗 **双向数据绑定**：Data Model 与 UI 组件自动同步，响应式传播变更
- 🧩 **18 个标准组件 + 14 个函数**：完整实现 Basic Catalog 的全部组件和函数
- 🔌 **自定义扩展**：`CustomComponentRegistry` 支持注册超出标准目录的自定义组件
- 🔒 **安全执行边界**：严格的 `callableFrom` 函数执行边界 enforcement
- 🧠 **增量依赖追踪**：`DependencyGraph` 精确追踪组件对 Data Model 的依赖，按需重渲染
- 🗂️ **Surface 生命周期管理**：状态机驱动的创建/活跃/销毁，LRU 自动驱逐

## 架构总览

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐
│   Agent      │────▶│  Transport Layer │────▶│  Protocol Core   │
│  （服务端）   │     │（AG-UI/A2A/MCP） │     │（消息解析/状态机） │
└─────────────┘     └──────────────────┘     └────────┬─────────┘
                                                      │
                                    ┌─────────────────┼─────────────────┐
                                    ▼                 ▼                 ▼
                              ┌──────────┐     ┌──────────┐     ┌──────────┐
                              │ TUI 渲染器│     │ GUI 渲染器│     │ Web 渲染器│
                              │（ratatui）│     │ （egui）  │     │（HTML/SSR）│
                              └──────────┘     └──────────┘     └──────────┘
                                    │                 │                 │
                                    ▼                 ▼                 ▼
                              终端界面           桌面窗口           浏览器页面
```

协议与渲染实现完全分离。传输层将 JSON 消息流送入协议核心，核心层维护 Surface 状态并调用 `Renderer` trait，各平台 crate 负责实际渲染输出。

## Workspace Crate 说明

| Crate | 职责 | 关键依赖 | 测试数 |
|-------|------|----------|--------|
| [`a2ui-core`](crates/a2ui-core/) | 协议类型定义、消息枚举、JSON Schema 解析、Surface 状态机 | `serde`, `serde_json`, `thiserror` | 121 |
| [`a2ui-renderer`](crates/a2ui-renderer/) | `Renderer` trait、组件树管理、数据绑定、路径解析、函数调度、DependencyGraph、SurfaceLru、CustomComponentRegistry | `a2ui-core` | 99 |
| [`a2ui-renderer-tui`](crates/a2ui-renderer-tui/) | 终端渲染器（ratatui + crossterm） | `ratatui 0.26`, `crossterm 0.27`, `a2ui-renderer` | 66 |
| [`a2ui-renderer-gui`](crates/a2ui-renderer-gui/) | 桌面渲染器（egui + eframe），原生窗口 + 即时模式 GUI | `egui 0.27`, `eframe 0.27`, `a2ui-renderer` | 40 |
| [`a2ui-renderer-web`](crates/a2ui-renderer-web/) | Web 渲染器，服务端渲染 HTML/CSS，XSS 安全转义 | `a2ui-renderer` | 54 |
| [`a2ui-transport`](crates/a2ui-transport/) | 传输层抽象 trait + WebSocket 绑定、能力协商握手 | `tokio`, `tokio-tungstenite`, `a2ui-core` | 13 |
| [`a2ui-cli`](crates/a2ui-cli/) | 命令行入口，集成 transport + renderer | `clap`, `ratatui`, `a2ui-transport` | 1 |

**架构约束：**

- `a2ui-transport` 只负责消息收发和会话管理，不包含渲染逻辑
- `a2ui-renderer` 定义 `Renderer` trait，各平台渲染器实现该 trait
- `a2ui-core` 是唯一直接依赖 `serde_json` 的 crate，下游通过 Rust 类型交互
- Surface 生命周期严格由状态机管理：`createSurface` → `Active` → `deleteSurface`

## 快速开始

### 环境要求

- Rust 1.75+
- 现代终端（TUI 渲染器需要 ANSI 支持）
- 桌面环境（GUI 渲染器需要系统窗口支持）

### 安装

```bash
git clone https://github.com/yufeng108/a2ui-rs.git
cd a2ui-rs
```

### 编译

```bash
# 编译整个 workspace
cargo build --workspace

# 编译指定 crate
cargo build -p a2ui-core
```

### 运行测试

```bash
# 运行全部测试（394 个）
cargo test --workspace

# 运行指定 crate
cargo test -p a2ui-renderer

# 格式化与静态检查
cargo fmt && cargo clippy --workspace
```

### 使用 CLI

从 STDIN 读取 JSONL 流并渲染到终端：

```bash
echo '{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic","sendDataModel":false}}' \
  | cargo run --bin a2ui -- render
```

## 运行示例

项目为三种渲染器都提供了交互式演示示例：

```bash
# TUI 终端渲染器（交互式按键操作，q/Esc 退出）
cargo run --example simple_tui -p a2ui-renderer-tui

# GUI 桌面渲染器（原生窗口，所有 18 个组件）
cargo run --example simple_gui -p a2ui-renderer-gui

# Web 渲染器（生成 HTML 文件）
cargo run --example simple_web -p a2ui-renderer-web
# 输出：a2ui_demo.html，浏览器打开即可查看
```

### TUI 示例

终端渲染器演示展示了 Text、TextField、CheckBox、Slider、Button、Divider 等组件的完整 TUI 渲染，支持 `q` / `Esc` 退出。

### GUI 示例

桌面窗口渲染器展示了所有 18 个 Basic Catalog 组件的原生 UI 渲染，包括输入控件的交互操作。

### Web 示例

Web 渲染器生成一个完整的语义化 HTML 页面，包含表单、卡片、列表、按钮等组件，所有输出经过 HTML 转义（XSS 安全）。

## 协议支持

### 消息类型

| 消息 | 方向 | 说明 | 状态 |
|------|------|------|:----:|
| `createSurface` | Server → Client | 创建 UI Surface，可内嵌组件树和 data model | ✅ |
| `updateComponents` | Server → Client | 邻接表式增量更新组件 | ✅ |
| `updateDataModel` | Server → Client | JSON Pointer 路径 upsert 数据 | ✅ |
| `deleteSurface` | Server → Client | 销毁 Surface 并清理资源 | ✅ |
| `actionResponse` | Server → Client | 服务端响应客户端 action | ✅ |
| `callFunction` | Server → Client | 服务端调用客户端注册函数 | ✅ |
| `action` | Client → Server | 用户交互事件 | ✅ |
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

组件以扁平邻接表形式按任意顺序到达，渲染器自动缓冲并渐进构建。已到达且引用完整的组件立即渲染，引用缺失的组件渲染为平台特定占位符（TUI 用虚线框、GUI 用灰色区域、Web 用 skeleton），后续到达后自动替换为真实内容。

```text
updateComponents: [root, title, button_label, button]
                        │
                        ▼
              root 定义 children: [title, button]
              title 已到达 → 渲染 Text
              button 引用 button_label（已到达） → 渲染 Button
```

### 双向数据绑定

输入组件（TextField、CheckBox、Slider 等）与 Data Model 建立双向绑定：

1. **读（Model → View）**：组件渲染时从绑定的 JSON Pointer 路径读取值
2. **写（View → Model）**：用户交互时立即更新本地 Data Model 对应路径
3. **响应性**：一个路径的变更自动传播到所有绑定该路径的组件

### DependencyGraph 增量重渲染

采用声明式依赖追踪而非全文刷新：Data Model 更新时精确标记哪些 Surface 需要重渲染，只有依赖了变更路径的组件才参与下一帧渲染。

```text
DataModel 更新 /user/name → "Alice"
        │
        ▼
DependencyGraph 查询哪些组件依赖 /user/name
        │
        ├── Text(id="name", text={path:"/user/name"})       → 需要重渲染
        ├── Text(id="title", text="固定标题")                 → 不需要
        └── Button(id="submit", checks=[...path:/user/name]) → 需要
```

### Surface 生命周期管理

采用状态机管理每个 Surface 的完整生命周期：

```text
未创建 → createSurface → Active（可接收 updateComponents / updateDataModel）
                                           │
                                           ▼
                                       deleteSurface → Deleted（不可恢复）
```

- Surface ID 在渲染器生命周期内全局唯一
- 支持 `SurfaceLru` 自动驱逐（计数上限 + 空闲超时）
- 删除时完整清理：组件树、Data Model、依赖图、占位符

### 自定义组件扩展

通过 `CustomComponentRegistry` 注册超出 Basic Catalog 的自定义组件类型，遇到未知组件类型时先查注册表，找到则正常渲染，未找到则显示占位符。

### 安全机制

- **函数执行边界 enforcement**：严格校验 `callableFrom`，未注册函数调用被拒绝
- **标识符校验**：所有 `ComponentId` 通过 Unicode UAX #31 命名规则校验
- **XSS 防护**：Web 渲染器对所有输出做 HTML 转义
- **JSON Pointer 边界检查**：防止路径逃出 Data Model 根范围
- **DoS 防护**：Surface 数量上限、组件树大小限制、递归深度限制

## 开发指南

### TDD 开发模式

项目严格遵循 **红 → 绿 → 重构** 的 TDD 循环：

1. **红**：先写一个失败的测试，明确描述期望行为
2. **绿**：用最少的代码让测试通过
3. **重构**：在测试始终通过的前提下优化代码结构和设计

### 测试分层

- **单元测试**（`#[cfg(test)]`）：放在 `src/` 源码文件末尾，测试单个函数或模块
- **集成测试**（`tests/` 目录）：从 crate 外部调用公开 API，验证模块间交互
- **文档测试**（`///` 代码块）：确保文档中的示例始终可编译运行

### 代码风格

- 错误处理使用 `thiserror` 定义 crate 级错误类型
- 异步接口统一使用 `async fn`
- 所有公共 API 必须有文档注释（`///`）
- 序列化统一使用 `serde` + `serde_json`

### 常用命令

```bash
# 编译所有成员
cargo build --workspace

# 运行所有测试
cargo test --workspace

# 运行单个测试
cargo test -p <package_name> <test_name>

# 格式化 + 静态检查
cargo fmt && cargo clippy --workspace
```

## 测试

当前测试覆盖情况：

| Crate | 单元测试 | 集成测试 | E2E | 文档测试 | 合计 |
|-------|:-------:|:-------:|:---:|:-------:|:---:|
| a2ui-core | 115 | 6 | — | — | **121** |
| a2ui-renderer | 99 | — | — | — | **99** |
| a2ui-renderer-gui | 37 | — | 3 | — | **40** |
| a2ui-renderer-tui | 63 | — | 3 | — | **66** |
| a2ui-renderer-web | 43 | — | 8 | 3 | **54** |
| a2ui-transport | 12 | 1 | — | — | **13** |
| a2ui-cli | — | 1 | — | — | **1** |
| **总计** | **369** | **8** | **14** | **3** | **394** |

```bash
# 验证全部通过
cargo test --workspace
# 输出：test result: ok. 0 failed
```

## 项目结构

```
a2ui-rs/
├── ARCHITECTURE.md                    # 架构设计文档（中文）
├── CLAUDE.md                          # Claude Code 协作指引
├── Cargo.toml                         # Workspace 配置
├── README.md                          # 本文件
├── crates/
│   ├── a2ui-core/                     # 协议核心
│   │   └── src/
│   │       ├── message/               # 消息类型定义（envelope、server_to_client、client_to_server）
│   │       ├── component/             # Component、Catalog、ChildList
│   │       ├── datamodel/             # Data Model 包装
│   │       ├── schema/                # JSON Schema 校验
│   │       └── state/                 # Surface 状态机
│   ├── a2ui-renderer/                 # 渲染器核心
│   │   └── src/
│   │       ├── trait.rs               # Renderer trait 定义
│   │       ├── component_tree.rs      # 组件树管理
│   │       ├── data_model.rs          # Data Model 存储
│   │       ├── dependency_graph.rs    # 增量依赖追踪
│   │       ├── function_dispatcher.rs # 函数调度（14 个函数）
│   │       ├── surface_lru.rs         # Surface LRU 驱逐
│   │       └── custom_component.rs    # 自定义组件注册表
│   ├── a2ui-renderer-tui/            # TUI 渲染器（ratatui）
│   │   ├── src/
│   │   │   ├── tui_renderer.rs        # Renderer trait 实现
│   │   │   └── widget_builder.rs      # ratatui widget 构建器
│   │   └── examples/simple_tui.rs     # TUI 交互演示
│   ├── a2ui-renderer-gui/            # GUI 桌面渲染器（egui）
│   │   ├── src/
│   │   │   ├── gui_renderer.rs        # Renderer trait 实现
│   │   │   ├── widget_mapper.rs       # 18 组件 → egui widget 映射
│   │   │   └── app.rs                 # eframe App 集成
│   │   └── examples/simple_gui.rs     # GUI 窗口演示
│   ├── a2ui-renderer-web/            # Web 渲染器（SSR HTML）
│   │   ├── src/
│   │   │   ├── web_renderer.rs        # Renderer trait 实现
│   │   │   └── html_builder.rs        # HTML/CSS 构建器（XSS 安全）
│   │   └── examples/simple_web.rs     # Web 演示（生成 HTML）
│   ├── a2ui-transport/               # 传输层
│   │   └── src/
│   │       ├── trait.rs               # Transport trait
│   │       ├── websocket.rs           # WebSocket 实现
│   │       └── capabilities.rs        # 能力协商握手
│   └── a2ui-cli/                      # 命令行工具
│       └── src/main.rs
└── docs/
    └── superpowers/
        └── plans/                     # 开发计划文档
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
  <sub>Built with 🦀 Rust · A2UI Protocol v1.0 · 2026</sub>
</div>
