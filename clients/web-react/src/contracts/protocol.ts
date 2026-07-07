// A2UI Protocol v1.0 —— 线格式类型（浏览器端）
//
// 这是 C / V / K 三条轨道共享的「协议 JSON 形态」契约，尽量贴合 Rust a2ui-core
// 的 serde 定义。轨道内实现只依赖这些类型，不各自重定义。
//
// 注意：本文件仅定义**数据形态**，不含任何运行时逻辑。

/** 协议版本字符串（如 "v1.0"）。用字符串而非枚举以便向前兼容。 */
export type ProtocolVersion = string;

/** 函数执行边界。 */
export type CallableFrom = "clientOnly" | "remoteOnly" | "clientOrRemote";

/** 组件 ID。 */
export type ComponentId = string;

/** Surface ID。 */
export type SurfaceId = string;

// ─── Dynamic 值：字面量 | 数据绑定 | 函数调用 ────────────────────────────────

/** 数据绑定：指向 Data Model 的 JSON Pointer。 */
export interface DataBinding {
  path: string;
}

/** 函数调用：调用已注册函数。 */
export interface FunctionCall {
  call: string;
  args?: Record<string, unknown>;
}

/**
 * 动态属性的三种形式：字面量 T / `{path}` 绑定 / `{call,args}` 函数调用。
 * 判别方式：对象且含 `path` → 绑定；含 `call` → 函数调用；否则字面量。
 */
export type DynamicValue<T> = T | DataBinding | FunctionCall;

export type DynamicString = DynamicValue<string>;
export type DynamicNumber = DynamicValue<number>;
export type DynamicBoolean = DynamicValue<boolean>;
export type DynamicStringList = DynamicValue<string[]>;

// ─── ChildList：静态数组 | 动态模板 ─────────────────────────────────────────

/** 动态模板：遍历 `path` 指向的数组，对每项实例化 `template` 组件。 */
export interface ChildListTemplate {
  template: ComponentId;
  path: string;
}

/** 子组件列表：静态 ID 数组，或动态模板对象。 */
export type ChildList = ComponentId[] | ChildListTemplate;

// ─── Action：事件回传 | 本地函数 ────────────────────────────────────────────

/**
 * 事件声明体（规范 · Server actions 的 `action.event` 内层）。
 * `responsePath` 是**客户端本地语义**（响应写回路径），不会出现在 wire 消息中。
 */
export interface ActionEvent {
  name: string;
  context?: Record<string, DynamicValue<unknown>>;
  wantResponse?: boolean;
  responsePath?: string;
  actionId?: string;
}

/** 执行本地注册函数（`action.functionCall` 内层）。 */
export interface ActionFunctionCall {
  call: string;
  args?: Record<string, DynamicValue<unknown>>;
}

/**
 * 按钮等交互组件的 `action` 属性（规范嵌套格式）：
 * `{ event: {...} }` 发送事件到服务端；`{ functionCall: {...} }` 本地函数调用。
 */
export type Action =
  | { event: ActionEvent }
  | { functionCall: ActionFunctionCall };

// ─── 组件（邻接表节点） ─────────────────────────────────────────────────────

/**
 * 组件为扁平列表 + id 引用（邻接表）。`component` 是类型判别符（如 "Text"）。
 * 其余属性因组件类型而异，故用宽松索引签名；轨道内按 `component` 收窄。
 */
export interface Component {
  id: ComponentId;
  component: string;
  /** ComponentCommon 之外的组件特有属性（text/value/children/action/...）。 */
  [key: string]: unknown;
}

// ─── 服务端 → 客户端消息 ────────────────────────────────────────────────────

export interface CreateSurface {
  surfaceId: SurfaceId;
  catalogId: string;
  surfaceProperties?: Record<string, unknown>;
  sendDataModel?: boolean;
  components?: Component[];
  dataModel?: unknown;
}

export interface UpdateComponents {
  surfaceId: SurfaceId;
  components: Component[];
}

export interface UpdateDataModel {
  surfaceId: SurfaceId;
  /** 省略或 "/" 表示替换整个 data model。 */
  path?: string;
  /** 省略表示删除 path 对应的 key。 */
  value?: unknown;
}

export interface DeleteSurface {
  surfaceId: SurfaceId;
}

export interface ActionResponseError {
  code: string;
  message: string;
}

export interface ActionResponse {
  actionId: string;
  value?: unknown;
  error?: ActionResponseError;
}

export interface CallFunction {
  functionCallId: string;
  wantResponse?: boolean;
  call: string;
  args?: Record<string, unknown>;
}

/** v1.0 服务端消息联合（一条 envelope 恰含其一）。 */
export interface V1_0ServerMessage {
  version?: ProtocolVersion;
  createSurface?: CreateSurface;
  updateComponents?: UpdateComponents;
  updateDataModel?: UpdateDataModel;
  deleteSurface?: DeleteSurface;
  actionId?: string;
  actionResponse?: Omit<ActionResponse, "actionId">;
  functionCallId?: string;
  wantResponse?: boolean;
  callFunction?: CallFunction;
}

/** 服务端信封（Agent → Renderer）。 */
export type ServerEnvelope = V1_0ServerMessage;

// ─── 客户端 → 服务端消息 ────────────────────────────────────────────────────

/**
 * 客户端 action 消息（规范 · action 消息 Properties）。
 * `name` / `surfaceId` / `sourceComponentId` / `timestamp` 均必填；
 * `actionId` 在 `wantResponse=true` 时必填（声明缺失时由客户端自动生成）。
 * 注意：声明里的 `responsePath` 是本地语义，**不在本消息中序列化**。
 */
export interface ActionMessage {
  name: string;
  surfaceId: SurfaceId;
  sourceComponentId: ComponentId;
  /** 事件发生时刻，ISO 8601 UTC（秒精度，`YYYY-MM-DDTHH:MM:SSZ`）。 */
  timestamp: string;
  context?: Record<string, unknown>;
  wantResponse?: boolean;
  actionId?: string;
}

export interface FunctionResponseMessage {
  functionCallId: string;
  call: string;
  value?: unknown;
}

export interface ErrorMessage {
  code: string;
  message: string;
  functionCallId?: string;
}

/** 客户端信封（Renderer → Agent）。 */
export interface ClientEnvelope {
  version?: ProtocolVersion;
  action?: ActionMessage;
  functionResponse?: FunctionResponseMessage;
  error?: ErrorMessage;
  /** sendDataModel 为真时随 action/functionResponse 附带的客户端状态。 */
  metadata?: ClientMetadata;
}

/** 当 Surface 的 sendDataModel 为真时，随消息附带的 data model 快照。 */
export interface ClientMetadata {
  surfaceId: SurfaceId;
  dataModel?: unknown;
}

// ─── 校验错误（输入组件 checks 失败） ───────────────────────────────────────

export interface CheckError {
  message: string;
  componentId: ComponentId;
  checkIndex: number;
}
