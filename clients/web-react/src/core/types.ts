/**
 * A2UI Protocol v1.0 的 TypeScript 协议类型。
 *
 * 这些类型对齐 Rust `a2ui-core` 的 serde 定义，决定浏览器端消费的 JSON 形态。
 * 信封为外部标签形式：`{ version: "v1.0", <messageKey>: {...} }`，其中
 * `messageKey` 为 camelCase 的消息变体名（如 `createSurface`）。
 *
 * 本模块只声明**形状**（type-only），运行期的解析/分发见
 * {@link "@/core/messages"}。
 */
import type { Json } from "@/core/json-pointer";

export type { Json };

/** 组件标识符（UAX #31 命名，`@` 命名空间保留给系统）。 */
export type ComponentId = string;

/** 协议版本字符串。当前仅支持 `"v1.0"`。 */
export const PROTOCOL_VERSION = "v1.0" as const;
export type ProtocolVersion = typeof PROTOCOL_VERSION;

// ---------------------------------------------------------------------------
// DynamicValue — 字面量 / 路径绑定 / 函数调用
// ---------------------------------------------------------------------------

/** 指向 Data Model 的路径绑定：`{ "path": "/foo/bar" }`。 */
export interface DataBinding {
  path: string;
}

/** 函数调用：`{ "call": "formatString", "args": {...} }`。 */
export interface FunctionCallValue {
  call: string;
  args?: Json;
}

/**
 * 动态值：字面量 `T`、路径绑定或函数调用三选一（untagged）。
 * 在组件属性里，字面量直接是标量（string/number/bool）。
 */
export type DynamicValue<T = Json> = T | DataBinding | FunctionCallValue;

// ---------------------------------------------------------------------------
// ChildList — 静态数组 / 动态模板
// ---------------------------------------------------------------------------

/** 动态模板 ChildList：遍历 `path` 指向的数组，对每项实例化 `template`。 */
export interface TemplateChildList {
  template: ComponentId;
  path: string;
}

/**
 * ChildList 两种模式：
 * - 静态：`ComponentId[]`（在组件属性里即 `children: [...]`）。
 * - 动态：{@link TemplateChildList}。
 */
export type ChildList = ComponentId[] | TemplateChildList;

// ---------------------------------------------------------------------------
// Component — 邻接表节点
// ---------------------------------------------------------------------------

/** 无障碍属性。 */
export interface AccessibilityAttributes {
  label?: string;
  description?: string;
}

/**
 * 组件：扁平列表中的一项。`id` + `component`（类型名）为固定字段，
 * 其余为该类型的特有属性（由 Catalog schema 定义），以 `properties` 承载。
 */
export interface Component {
  id: ComponentId;
  /** 组件类型名，如 `"Text"`、`"Button"`、`"Column"`。 */
  component: string;
  accessibility?: AccessibilityAttributes;
  /** flex-grow 风格权重，仅在 Row/Column 直接子组件时有效。 */
  weight?: number;
  /** 除固定字段外的全部特有属性（`text`/`value`/`children`/`action`…）。 */
  properties: Record<string, Json>;
}

// ---------------------------------------------------------------------------
// Action — 事件 / 本地函数调用
// ---------------------------------------------------------------------------

/** 事件型 action（组件属性里的形态，untagged：有 `name`）。 */
export interface EventActionSpec {
  name: string;
  context?: Record<string, DynamicValue>;
  wantResponse?: boolean;
  responsePath?: string;
  actionId?: string;
}

/** 本地函数调用型 action（untagged：有 `call`）。 */
export interface FunctionCallActionSpec {
  call: string;
  args?: Record<string, DynamicValue>;
}

/** 组件 `action` 属性：事件或本地函数调用。 */
export type ActionSpec = EventActionSpec | FunctionCallActionSpec;

// ---------------------------------------------------------------------------
// Server → Client 消息
// ---------------------------------------------------------------------------

export interface CreateSurface {
  surfaceId: string;
  catalogId: string;
  surfaceProperties?: Json;
  sendDataModel?: boolean;
  components?: Component[];
  dataModel?: Json;
}

export interface UpdateComponents {
  surfaceId: string;
  components: Component[];
}

export interface UpdateDataModel {
  surfaceId: string;
  /** 省略或 `"/"` 表示整个 model。 */
  path?: string;
  /** 省略（字段缺失）表示删除该路径；显式 `null` 表示置空。 */
  value?: Json;
  /** 反序列化时是否显式带了 `value` 字段（区分删除 vs 置 null）。 */
  hasValue: boolean;
}

export interface DeleteSurface {
  surfaceId: string;
}

export interface ResponseError {
  code: string;
  message: string;
}

export interface ActionResponse {
  actionId: string;
  /** 成功时的返回值（与 `error` 互斥）。 */
  value?: Json;
  /** 失败时的错误（与 `value` 互斥）。 */
  error?: ResponseError;
}

export interface CallFunction {
  functionCallId: string;
  wantResponse: boolean;
  call: string;
  args?: Json;
}

/** 服务端 → 客户端消息的判别联合。 */
export type ServerMessage =
  | { kind: "createSurface"; message: CreateSurface }
  | { kind: "updateComponents"; message: UpdateComponents }
  | { kind: "updateDataModel"; message: UpdateDataModel }
  | { kind: "deleteSurface"; message: DeleteSurface }
  | { kind: "actionResponse"; message: ActionResponse }
  | { kind: "callFunction"; message: CallFunction };

export type ServerMessageKind = ServerMessage["kind"];

// ---------------------------------------------------------------------------
// Client → Server 消息
// ---------------------------------------------------------------------------

export interface ActionMessage {
  name: string;
  surfaceId: string;
  sourceComponentId?: string;
  context?: Record<string, Json>;
  wantResponse?: boolean;
  responsePath?: string;
  actionId?: string;
}

export interface FunctionResponse {
  functionCallId: string;
  call: string;
  value: Json;
}

export interface ClientError {
  code: string;
  message: string;
  functionCallId?: string;
}

/** 客户端 → 服务端信封（外部标签，`version` 邻接）。 */
export type ClientEnvelope =
  | { version: ProtocolVersion; action: ActionMessage }
  | { version: ProtocolVersion; functionResponse: FunctionResponse }
  | { version: ProtocolVersion; error: ClientError };

// ---------------------------------------------------------------------------
// 判别辅助
// ---------------------------------------------------------------------------

/** 判断动态值是否为路径绑定 `{ path }`（且非模板 `{ template, path }`）。 */
export function isDataBinding(v: unknown): v is DataBinding {
  return (
    typeof v === "object" &&
    v !== null &&
    !Array.isArray(v) &&
    typeof (v as Record<string, unknown>).path === "string" &&
    typeof (v as Record<string, unknown>).template === "undefined"
  );
}

/** 判断动态值是否为函数调用 `{ call, args? }`。 */
export function isFunctionCall(v: unknown): v is FunctionCallValue {
  return (
    typeof v === "object" &&
    v !== null &&
    !Array.isArray(v) &&
    typeof (v as Record<string, unknown>).call === "string"
  );
}

/** 判断 ChildList 是否为动态模板 `{ template, path }`。 */
export function isTemplateChildList(v: unknown): v is TemplateChildList {
  return (
    typeof v === "object" &&
    v !== null &&
    !Array.isArray(v) &&
    typeof (v as Record<string, unknown>).template === "string" &&
    typeof (v as Record<string, unknown>).path === "string"
  );
}

/** 判断 action 规格是否为函数调用型。 */
export function isFunctionCallAction(
  a: ActionSpec,
): a is FunctionCallActionSpec {
  return typeof (a as FunctionCallActionSpec).call === "string";
}
