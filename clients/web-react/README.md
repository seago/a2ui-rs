# a2ui web-react（B2 交互式 Web 渲染器）

A2UI Protocol v1.0 的浏览器端**交互式**渲染器：把 Agent 推送的 A2UI 协议消息流
渲染为可交互界面，并把用户交互回传服务端。与 Rust 侧的 `a2ui-renderer-web`
（静态派，输出 HTML 字符串）互补，本项目是**交互派**。

## 架构：协议核心 + 可插拔 ComponentKit

```
WS/SSE ──▶ ① 协议核心(纯 TS)  ──▶ ② 渲染核心(React, 库无关) ──▶ ③ ComponentKit
           src/core                src/react                    src/kits/*
           SurfaceStore /           A2UIProvider /               shadcn / html ...
           DataModel / 响应性        Surface / tree-walker
```

- **`src/contracts`** — 冻结的接口缝：协议类型、`SurfaceStore`、`ComponentKit` props。
- **`src/core`** — 纯 TS 协议引擎：消息解析、JSON Pointer DataModel、组件森林、
  作用域路径解析（含 `@index`）、依赖图响应性、`formatString`、函数调度、交互回传。
- **`src/react`** — 库无关渲染核心：`A2UIProvider` / `Surface` / tree-walker。
- **`src/kits`** — 可插拔组件库实现：
  - `shadcn/` — 基于 shadcn/ui 的 kit（首选）。
  - `html/` — 纯 HTML kit（证明"换 kit 换库"的第二实现）。
- **`src/transport`** — 浏览器 WS 客户端。

核心原则：**换整个组件库只需给 `<A2UIProvider kit={...}>` 传一个不同的 kit**，
协议核心与渲染核心零改动，Data Model 状态跨切换保留。

## 命令

```bash
npm install          # 安装依赖
npm run dev          # 开发服务器（App，连 ws://127.0.0.1:8765）
npm run demo         # 打开可插拔 kit 示例页（/demo.html，无需 WS 服务端）
npm test             # 运行全部单元/集成测试（Vitest）
npm run build        # 类型检查 + 生产构建
```

## 示例页（`npm run demo`）

`demo.html` + `src/demo/` 是一个自包含示例：把一条内嵌的 A2UI `createSurface`
喂进真实协议核心 store，用顶部开关**实时切换 shadcn / 纯 HTML kit**。
同一套协议消息在两套组件库下渲染，表单内容跨切换保留——直观演示可插拔缝。

## 端到端连真实服务端

`npm run dev` 的 App 连 `ws://127.0.0.1:8765`。配套的 Rust 演示服务端：

```bash
cargo run -p a2ui-transport --example serve_demo
```

## 防漂移

`../../tests/conformance/*.json` 是语言无关的协议一致性向量，由本项目
（`src/core/*conformance*.test.ts`）与 Rust 侧（`crates/a2ui-renderer/tests/conformance.rs`）
同跑，确保两份协议实现语义一致。
