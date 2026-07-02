// ComponentKit 契约 —— 渲染核心（轨道 V）与组件库实现（轨道 K）之间的缝。
//
// 关键原则：props 契约由 A2UI 协议定义，**不由任何一个组件库定义**。
// shadcn kit、未来的 MUI kit 都实现同一套 props；切库只换 kit，核心层不动。
// V 负责把 SurfaceStore 的 ResolvedNode 映射成这些 props 并接线回调。

import type { FC, ReactNode } from "react";
import type { CheckError } from "./protocol";

/** Text：文本显示。 */
export interface TextProps {
  text: string;
  variant: "body" | "caption";
}

/** Button：按钮。label 可含子节点（如内嵌 Text/Icon，由 V 渲染后传入）。 */
export interface ButtonProps {
  label: ReactNode;
  variant: "default" | "primary" | "borderless";
  disabled: boolean;
  /** 点击时触发（V 已封装为「更新 Data Model + 回传 action / 执行本地函数」）。 */
  onAction: () => void;
}

/** TextField：文本输入，与 Data Model 双向绑定。 */
export interface TextFieldProps {
  value: string;
  /** 输入变化时立即写回本地 Data Model（V 已接线）。 */
  onChange: (value: string) => void;
  label?: string;
  placeholder?: string;
  variant: "shortText" | "number" | "longText" | "obscured";
  disabled: boolean;
  /** checks 失败明细，供 kit 展示错误态。 */
  errors: CheckError[];
}

/** Card：单子组件容器。 */
export interface CardProps {
  children: ReactNode;
}

/** Column：纵向布局容器。 */
export interface ColumnProps {
  children: ReactNode;
}

/** Row：横向布局容器。 */
export interface RowProps {
  children: ReactNode;
}

/** Placeholder：未知类型 / 引用缺失的兜底。 */
export interface PlaceholderProps {
  reason: string;
}

/**
 * 各 A2UI 组件类型的归一化 props 映射。M1 骨架先覆盖四个组件 + 占位符；
 * M2 按此模式扩展到 18 个 Basic Catalog 组件。
 */
export interface ComponentKitProps {
  Text: TextProps;
  Button: ButtonProps;
  TextField: TextFieldProps;
  Card: CardProps;
  Column: ColumnProps;
  Row: RowProps;
  Placeholder: PlaceholderProps;
}

/** 已知组件类型名。 */
export type KnownComponent = keyof ComponentKitProps;

/**
 * 一个 ComponentKit = 每个已知 A2UI 组件类型 → 一个实现该 props 契约的 React 组件。
 * 渲染核心按 `component` 名查表；未命中（未知/自定义类型）时回退到 `Placeholder`。
 * 自定义组件扩展留待 M2/M3，届时再加可选的扩展映射，不影响此契约。
 */
export type ComponentKit = {
  [K in KnownComponent]: FC<ComponentKitProps[K]>;
};
