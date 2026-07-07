// SurfaceStore 契约 —— 协议核心层（轨道 C 实现）与渲染核心（轨道 V 消费）之间的缝。
//
// V 只依赖这个接口，测试时用 mock 实现；C 提供真实实现。
// 二者都不各自重定义协议类型——统一从 ./protocol 引入。

import type {
  Action,
  CheckError,
  ClientEnvelope,
  ComponentId,
  ServerEnvelope,
  SurfaceId,
} from "./protocol";

/** Surface 生命周期状态。 */
export type SurfaceLifecycle = "pending" | "active" | "deleted";

/**
 * 作用域：根作用域，或集合作用域（ChildList template 实例化时）。
 * 集合作用域携带数组基路径与当前索引链，用于相对路径解析与 `@index`。
 */
export interface Scope {
  /** 从根到当前的集合帧；空数组表示根作用域。 */
  frames: ReadonlyArray<CollectionFrame>;
}

export interface CollectionFrame {
  /** 数组在 Data Model 中的绝对基路径，如 "/items"。 */
  basePath: string;
  /** 当前项索引（0 起）。 */
  index: number;
}

/** 定位一个（组件 × 作用域）实例。同一 template 组件在不同项下是不同节点。 */
export interface NodeRef {
  componentId: ComponentId;
  scope: Scope;
}

/**
 * 组件解析后的视图节点：DynamicValue 已解析为具体值，ChildList 已展开为子 NodeRef。
 * V 据此递归渲染，并按 `component` 映射到 ComponentKit。
 */
export interface ResolvedNode {
  id: ComponentId;
  /** 组件类型，如 "Text" / "Button" / "TextField" / "Card"。 */
  component: string;
  /** 已解析的组件属性（字面量 / 绑定 / 函数调用都已求值为具体值）。 */
  props: Record<string, unknown>;
  /** 已展开的子节点引用（静态数组或 template 实例化的结果）。 */
  children: NodeRef[];
  /** 交互组件（如 Button）的 action，若有。 */
  action?: Action;
  /** 输入组件（TextField 等）双向绑定的**绝对** data 路径，供写回。 */
  bindingPath?: string;
  /** checks 求值后是否禁用（如按钮校验未过）。 */
  disabled?: boolean;
  /** checks 失败明细。 */
  errors?: CheckError[];
  /** 引用缺失、类型未知等需要渲染占位符的原因；非空时 V 渲染 Placeholder。 */
  placeholder?: string;
}

/** 单个 Surface 的只读快照。 */
export interface SurfaceSnapshot {
  surfaceId: SurfaceId;
  catalogId: string;
  lifecycle: SurfaceLifecycle;
  /** root 组件引用；root 尚未到达时为 undefined。 */
  root?: NodeRef;
}

/**
 * 协议核心 Store。喂入服务端消息、维护组件森林 + Data Model + 响应性，
 * 对外提供解析后的视图与交互回传能力。**不依赖 React。**
 */
export interface SurfaceStore {
  /** 喂入一条服务端信封，更新内部状态并通知订阅者。 */
  ingest(envelope: ServerEnvelope): void;

  /** 当前所有 surface id。 */
  getSurfaceIds(): SurfaceId[];

  /** 取某 surface 快照。 */
  getSurface(surfaceId: SurfaceId): SurfaceSnapshot | undefined;

  /** 取 root 节点引用；不存在返回 undefined。 */
  getRootRef(surfaceId: SurfaceId): NodeRef | undefined;

  /** 解析某（组件 × 作用域）为视图节点；不存在返回 undefined。 */
  resolveNode(surfaceId: SurfaceId, ref: NodeRef): ResolvedNode | undefined;

  /** 读 Data Model 某绝对路径的值。 */
  getDataValue(surfaceId: SurfaceId, path: string): unknown;

  /**
   * View → Model 写回（输入组件交互时立即调用）。
   * path 为**绝对** data 路径（通常取自 ResolvedNode.bindingPath）。
   * 会更新 Data Model 并通知依赖该路径的订阅者。
   */
  setDataValue(surfaceId: SurfaceId, path: string, value: unknown): void;

  /**
   * 订阅任意状态变更（组件树 / Data Model）。返回取消订阅函数。
   * 供 React 层用 useSyncExternalStore 桥接。
   */
  subscribe(listener: () => void): () => void;

  /**
   * 由一个 Action 构造回传给服务端的客户端信封（含 sendDataModel 时的 metadata）。
   * `sourceComponentId` 必填（规范 action 消息必填字段）。
   * scope 用于解析 action.event.context 中的相对路径 / @index。
   */
  buildActionEnvelope(
    surfaceId: SurfaceId,
    action: Action,
    sourceComponentId: ComponentId,
    scope?: Scope,
  ): ClientEnvelope;
}

/** 根作用域常量，便于构造顶层 NodeRef。 */
export const ROOT_SCOPE: Scope = { frames: [] };
