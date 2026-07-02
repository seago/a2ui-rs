/**
 * DynamicValue / 组件属性的解析，含依赖路径收集（响应性的基础）。
 *
 * 解析规则（untagged DynamicValue）：
 * - 字面量（标量/数组/普通对象）→ 递归解析内部可能的动态值。
 * - `{ path }` → 从当前作用域解析，并登记其绝对路径为依赖。
 * - `{ call, args }` → 特判 `@index`；`formatString` 先解析 `bindings` 再插值；
 *   其余走 {@link FunctionDispatcher}（client 边界），未注册/失败则降级为 `undefined`。
 *
 * 结构性属性（children/child/content/trigger/tabs/action）不在此解析，由
 * SurfaceStore 处理树结构与交互。
 */
import {
  isDataBinding,
  isFunctionCall,
  type Json,
} from "@/core/types";
import type { Component } from "@/core/types";
import type { PathResolver } from "@/core/path-resolver";
import { FunctionError, type FunctionDispatcher } from "@/core/functions";
import { formatString } from "@/core/format-string";

/** 解析上下文：解析器 + 调度器 + 依赖收集集合。 */
export interface ResolveContext {
  resolver: PathResolver;
  dispatcher: FunctionDispatcher;
  /** 本次解析触达的绝对数据路径（去重）。 */
  deps: Set<string>;
}

/** 组件属性里不做值解析的结构性 key。 */
export const STRUCTURAL_KEYS: ReadonlySet<string> = new Set([
  "children",
  "child",
  "content",
  "trigger",
  "tabs",
  "action",
]);

function isPlainObject(v: Json): v is { [key: string]: Json } {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/**
 * 解析任意（可能内嵌动态值的）JSON 节点为具体值，收集依赖到 `ctx.deps`。
 * 未命中的路径 / 不可用的函数返回 `undefined`。
 */
export function resolveValue(node: Json, ctx: ResolveContext): Json | undefined {
  if (isDataBinding(node)) {
    ctx.deps.add(ctx.resolver.makeAbsolute(node.path));
    const v = ctx.resolver.resolve(node.path);
    return v === undefined ? undefined : v;
  }
  if (isFunctionCall(node)) {
    return resolveCall(node.call, node.args, ctx);
  }
  if (Array.isArray(node)) {
    return node.map((item) => resolveValue(item, ctx) ?? null);
  }
  if (isPlainObject(node)) {
    const out: { [key: string]: Json } = {};
    for (const [k, v] of Object.entries(node)) {
      const r = resolveValue(v, ctx);
      if (r !== undefined) out[k] = r;
    }
    return out;
  }
  return node;
}

function resolveCall(
  call: string,
  args: Json | undefined,
  ctx: ResolveContext,
): Json | undefined {
  if (call === "@index") {
    const idx = ctx.resolver.currentIndex();
    return idx === undefined ? undefined : idx;
  }

  const argObj = isPlainObject(args as Json) ? (args as { [k: string]: Json }) : {};

  if (call === "formatString") {
    const template = typeof argObj.template === "string" ? argObj.template : "";
    const bindingsRaw = isPlainObject(argObj.bindings as Json)
      ? (argObj.bindings as { [k: string]: Json })
      : {};
    const values: Record<string, Json> = {};
    for (const [k, v] of Object.entries(bindingsRaw)) {
      const r = resolveValue(v, ctx);
      values[k] = r === undefined ? "" : r;
    }
    return formatString(template, values);
  }

  // 其余函数：先解析参数中的动态值，再走调度器（client 边界）。
  const resolvedArgs: Record<string, Json> = {};
  for (const [k, v] of Object.entries(argObj)) {
    const r = resolveValue(v, ctx);
    if (r !== undefined) resolvedArgs[k] = r;
  }
  try {
    return ctx.dispatcher.dispatch(call, resolvedArgs, "client");
  } catch (e) {
    if (e instanceof FunctionError) return undefined;
    throw e;
  }
}

/** 一个组件解析后的视图属性 + 其依赖路径集合。 */
export interface ResolvedProps {
  props: Record<string, Json>;
  deps: Set<string>;
}

/**
 * 解析组件的非结构性属性为具体值，返回 props 与依赖集合。
 * 值为 `undefined`（未命中路径等）的属性被省略。
 */
export function resolveComponentProps(
  component: Component,
  resolver: PathResolver,
  dispatcher: FunctionDispatcher,
): ResolvedProps {
  const ctx: ResolveContext = { resolver, dispatcher, deps: new Set() };
  const props: Record<string, Json> = {};
  for (const [key, value] of Object.entries(component.properties)) {
    if (STRUCTURAL_KEYS.has(key)) continue;
    const r = resolveValue(value, ctx);
    if (r !== undefined) props[key] = r;
  }
  return { props, deps: ctx.deps };
}
