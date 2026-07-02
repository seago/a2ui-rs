# `src/kits/shadcn/` — shadcn ComponentKit

用 shadcn/ui 组件实现 `@/react` 定义的 `ComponentKit` 接口。

## 职责

- 把每种 A2UI 组件类型映射到对应的 shadcn/ui 渲染实现。
- 基础 UI 组件放在 `@/components/ui/*`（由 shadcn CLI 生成/维护，如 `button`），本目录负责「协议 → shadcn 组件」的适配逻辑。

## 约束

- 依赖 `@/react`（ComponentKit 接口）与 `@/components/ui/*`（shadcn 组件）。
- 不直接处理协议 JSON，只消费 `@/core` 的类型。

## 由谁填充

轨道 K（骨架四组件）。当前仅有占位导出 `SHADCN_KIT_PLACEHOLDER`，可安全替换。
