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

// ─── 显示组件 ────────────────────────────────────────────────────────────────

/** Image：图片展示。 */
export interface ImageProps {
  url: string;
  fit?: "contain" | "cover" | "fill";
  variant?: string;
}

/** Icon：矢量图标（name 为图标枚举名）。 */
export interface IconProps {
  name: string;
}

/** Video：视频播放器。 */
export interface VideoProps {
  url: string;
  posterUrl?: string;
}

/** AudioPlayer：音频播放器。 */
export interface AudioPlayerProps {
  url: string;
  description?: string;
}

/** Divider：分割线（无特有属性）。 */
export type DividerProps = Record<never, never>;

// ─── 容器组件 ────────────────────────────────────────────────────────────────

/** List：列表容器，可纵向/横向排列子项。 */
export interface ListProps {
  children: ReactNode;
  direction: "vertical" | "horizontal";
}

/** Tabs 的单个标签页（content 已由渲染核心渲染为 ReactNode）。 */
export interface TabItem {
  title: string;
  content: ReactNode;
}

/** Tabs：标签页容器。 */
export interface TabsProps {
  tabs: TabItem[];
}

/** Modal：模态框（trigger 触发，content 为内容，均已渲染为 ReactNode）。 */
export interface ModalProps {
  content: ReactNode;
  trigger: ReactNode;
}

// ─── 输入组件（与 Data Model 双向绑定） ──────────────────────────────────────

/** CheckBox：复选框。 */
export interface CheckBoxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  disabled: boolean;
}

/** Slider：滑块。 */
export interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  label?: string;
  disabled: boolean;
}

/** ChoicePicker 的单个选项。 */
export interface ChoiceOption {
  value: string;
  label: string;
}

/** ChoicePicker：选择器（多选 / 互斥单选）。 */
export interface ChoicePickerProps {
  value: string[];
  onChange: (value: string[]) => void;
  options: ChoiceOption[];
  variant: "multipleSelection" | "mutuallyExclusive";
  displayStyle: "checkbox" | "chips";
  disabled: boolean;
}

/** DateTimeInput：日期时间选择器。 */
export interface DateTimeInputProps {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  enableDate: boolean;
  enableTime: boolean;
  min?: string;
  max?: string;
  disabled: boolean;
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
  Image: ImageProps;
  Icon: IconProps;
  Video: VideoProps;
  AudioPlayer: AudioPlayerProps;
  Row: RowProps;
  Column: ColumnProps;
  List: ListProps;
  Card: CardProps;
  Tabs: TabsProps;
  Modal: ModalProps;
  Divider: DividerProps;
  Button: ButtonProps;
  TextField: TextFieldProps;
  CheckBox: CheckBoxProps;
  ChoicePicker: ChoicePickerProps;
  Slider: SliderProps;
  DateTimeInput: DateTimeInputProps;
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
