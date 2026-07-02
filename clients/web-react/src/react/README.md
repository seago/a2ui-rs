# `src/react/` — 渲染核心（React 库无关）

将 `@/core` 的协议数据模型渲染为 React 树。

## 职责

- 遍历 A2UI 组件树，把每个组件节点分发给已注册的 ComponentKit 渲染。
- 管理 Surface 到 React 组件的挂载 / 更新 / 卸载。
- 事件绑定：把用户交互回传为 `@/core` 定义的协议事件。
- 定义 `ComponentKit` 接口，供 `@/kits/*` 实现。

## 约束

- 依赖 React，但**不依赖任何具体组件库**（shadcn/ui、MUI 等）。具体组件实现全部下沉到 `@/kits/*`。
- 只通过 `@/core` 的类型与函数消费协议数据，不直接解析 JSON。

## 由谁填充

轨道 V。当前仅有占位导出 `REACT_RENDERER_PLACEHOLDER`，可安全替换。
