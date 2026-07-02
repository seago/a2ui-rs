# `src/core/` — 协议核心层（纯 TS）

对应 Rust 侧的 `a2ui-core` crate，是本前端工程唯一处理「A2UI 协议数据」的层。

## 职责

- 定义 A2UI v1.0 的消息类型（`createSurface` / `updateSurface` / `deleteSurface` / 事件回传等）。
- 定义 Surface、Component 树等数据模型的 TypeScript 类型。
- 提供 JSON 的解析 / 校验 / 序列化。
- Surface 生命周期状态机（`createSurface` → 活跃 → `deleteSurface`）。

## 约束

- **纯 TS**：不依赖 React、DOM 或任何渲染库。下游（`src/react/`、`src/kits/`）只依赖本层导出的类型与函数，不直接处理 JSON。
- 类型定义须与 A2UI 协议规范（<https://a2ui.org/specification/v1.0-a2ui/>）及 Rust 侧 `a2ui-core` 保持一致，不得引入冲突定义。

## 由谁填充

轨道 C。当前仅有占位导出 `CORE_PLACEHOLDER`，可安全替换。
