/**
 * A2UI 协议核心层（纯 TypeScript，无 React / DOM）。
 *
 * 这是 A2UI (Agent to UI) Protocol v1.0 的浏览器端协议引擎：消息解析/分发、
 * Data Model（JSON Pointer）、组件森林与生命周期、作用域路径解析、DynamicValue
 * 解析与依赖图响应性、formatString、函数调度、以及交互回传的信封构造。
 *
 * ## 给渲染层（Track V）的对接入口
 *
 * ```ts
 * import { A2uiEngine } from "@/core";
 *
 * const engine = new A2uiEngine();
 *
 * // 1) 喂消息（WS 收到的每条 ServerEnvelope，字符串或对象皆可）
 * engine.ingest(rawEnvelope);
 *
 * // 2) 取某 Surface 的解析结果
 * const surface = engine.getSurface("s1");
 * surface?.getRenderTree();          // 展开后的渲染树（含 template + @index）
 * surface?.resolveComponent("name_field"); // 单组件解析后的 props + 依赖
 * surface?.getDataValue("/form/name");     // 读 Data Model
 *
 * // 3) 双向绑定写回（用户输入）+ 订阅响应性
 * const off = surface?.subscribe(({ affected }) => rerender(affected));
 * surface?.setDataValue("/form/name", "张三");
 *
 * // 4) 交互回传（按钮点击）
 * const dispatch = engine.buildActionMessage("s1", "submit_btn");
 * if (dispatch) send(dispatch.envelope); // dispatch.dataModel 在 sendDataModel 时随附
 * ```
 */

// ─── 主入口：SurfaceStore 契约实现（Track V 只依赖此契约） ──────────────────
//
// `createSurfaceStore()` 返回满足 `@/contracts/store` 的 SurfaceStore 实例。
// 用法（对齐 store.ts 签名）：
//   const store = createSurfaceStore();
//   store.ingest(envelope);
//   const ref = store.getRootRef("s1");
//   const node = ref && store.resolveNode("s1", ref);
//   store.getDataValue("s1", "/form/name");
//   store.setDataValue("s1", "/form/name", "张三");
//   const off = store.subscribe(() => rerender());
//   const env = store.buildActionEnvelope("s1", node.action, "submit_btn", ref.scope);
export { createSurfaceStore } from "@/core/store";

// 顶层入口（内部引擎；SurfaceStore 之下的等价 API，保留以兼容既有测试与调试）
export { A2uiEngine } from "@/core/engine";
export type { IngestResult } from "@/core/engine";

// Surface 与解析结果
export { Surface, childRefsOf } from "@/core/surface";
export type {
  SurfaceState,
  ResolvedComponent,
  ResolvedNode,
  ChildRefs,
  ActionDispatch,
  ChangeNotification,
} from "@/core/surface";

// Data Model 与 JSON Pointer
export { DataModel } from "@/core/data-model";
export type { DataModelChange } from "@/core/data-model";
export {
  resolvePointer,
  applyPointer,
  validatePointer,
  escapeToken,
  unescapeToken,
  parseArrayIndex,
  isRootPointer,
  PointerError,
} from "@/core/json-pointer";
export type { Json } from "@/core/json-pointer";

// 消息解析
export { parseServerEnvelope, parseComponent } from "@/core/messages";
export type { ParseResult } from "@/core/messages";

// 解析引擎
export { PathResolver } from "@/core/path-resolver";
export type { Scope } from "@/core/path-resolver";
export {
  resolveValue,
  resolveComponentProps,
  STRUCTURAL_KEYS,
} from "@/core/resolve";
export type { ResolveContext, ResolvedProps } from "@/core/resolve";

// formatString 与函数调度
export { formatString, htmlEscape, valueToString } from "@/core/format-string";
export { FunctionDispatcher, FunctionError } from "@/core/functions";
export type { CallableFrom, CallSite, FunctionHandler } from "@/core/functions";

// 依赖图
export { DependencyGraph, pathsOverlap } from "@/core/dependency-graph";

// 判别辅助
export {
  isDataBinding,
  isFunctionCall,
  isTemplateChildList,
  isFunctionCallAction,
  PROTOCOL_VERSION,
} from "@/core/types";

// 协议类型
export type {
  ComponentId,
  ProtocolVersion,
  DynamicValue,
  DataBinding,
  FunctionCallValue,
  ChildList,
  TemplateChildList,
  Component,
  AccessibilityAttributes,
  ActionSpec,
  EventActionSpec,
  FunctionCallActionSpec,
  CreateSurface,
  UpdateComponents,
  UpdateDataModel,
  DeleteSurface,
  ActionResponse,
  ResponseError,
  CallFunction,
  ServerMessage,
  ServerMessageKind,
  ActionMessage,
  FunctionResponse,
  ClientError,
  ClientEnvelope,
} from "@/core/types";
