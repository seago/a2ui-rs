/**
 * `createSurfaceStore` —— A2UI Protocol v1.0 浏览器端协议核心层（纯 TS，无 React）。
 *
 * 实现冻结契约 {@link "@/contracts/store".SurfaceStore}：喂入 {@link ServerEnvelope}、
 * 维护组件森林 + Data Model + 响应性，对外提供惰性解析视图（{@link resolveNode}）与
 * 交互回传（{@link buildActionEnvelope}）。Track V（React 渲染核心）只依赖该契约。
 *
 * 设计要点（与 Rust 参考实现语义对齐）：
 * - **组件森林**：扁平 map + id 引用，惰性建树。root 未到达时缓冲；缺失引用产出带
 *   `placeholder` 的 {@link ResolvedNode}（渐进式渲染）。
 * - **惰性 NodeRef 解析**：{@link resolveNode} 按 (componentId × scope) 求值单个节点，
 *   ChildList 展开为子 {@link NodeRef}（静态数组原样；template 遍历数组、为每项压入
 *   一个 {@link CollectionFrame}）。React 据此递归渲染，天然支持局部重渲染。
 * - **作用域路径解析**：根作用域绝对路径 `"/…"`；集合作用域相对路径解析为
 *   `"/base/index/rel"`；`@index` 注入当前项索引。
 * - **响应性**：解析节点时把依赖的绝对路径登记进依赖图；数据变更后反查受影响者并
 *   通知订阅者（M1 用整体通知，依赖图已就绪，供 V 侧做精确失效）。
 *
 * @example
 * ```ts
 * import { createSurfaceStore } from "@/core/store";
 * const store = createSurfaceStore();
 * store.ingest({ version: "v1.0", createSurface: {
 *   surfaceId: "s1", catalogId: "basic",
 *   components: [{ id: "root", component: "Text", text: "hi" }],
 * }});
 * const ref = store.getRootRef("s1")!;
 * store.resolveNode("s1", ref)?.props.text; // "hi"
 * ```
 */
import type {
  Action,
  ActionEvent,
  ActionMessage,
  CheckError,
  ClientEnvelope,
  Component,
  ComponentId,
  ServerEnvelope,
  SurfaceId,
} from "@/contracts/protocol";
import type {
  CollectionFrame,
  NodeRef,
  ResolvedNode,
  Scope,
  SurfaceLifecycle,
  SurfaceSnapshot,
  SurfaceStore,
} from "@/contracts/store";
import { ROOT_SCOPE } from "@/contracts/store";

import { DataModel } from "@/core/data-model";
import { PathResolver } from "@/core/path-resolver";
import { FunctionDispatcher, FunctionError } from "@/core/functions";
import { DependencyGraph } from "@/core/dependency-graph";
import { formatString } from "@/core/format-string";
import type { Json } from "@/core/json-pointer";

// ─── 值/结构判别（针对 protocol.ts 的扁平线格式） ───────────────────────────

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** `{ path }`（且非 `{ template, path }` 的 ChildList）。 */
function isDataBinding(v: unknown): v is { path: string } {
  return isObject(v) && typeof v.path === "string" && v.template === undefined;
}

/** `{ call, args? }`。 */
function isFunctionCall(v: unknown): v is { call: string; args?: unknown } {
  return isObject(v) && typeof v.call === "string";
}

/** `{ template, path }`。 */
function isTemplateChildList(
  v: unknown,
): v is { template: ComponentId; path: string } {
  return (
    isObject(v) &&
    typeof v.template === "string" &&
    typeof v.path === "string"
  );
}

/** 事件型 action（有 `name`）。函数调用型（有 `call`）在回传时被忽略。 */
function isEventAction(a: Action): a is ActionEvent {
  return typeof (a as ActionEvent).name === "string";
}

/**
 * 组件属性里不做值求值的结构性 key（子引用与交互，由 store 处理树/回传）。
 * `id` / `component` 是判别符，同样跳过。
 */
const STRUCTURAL_KEYS: ReadonlySet<string> = new Set([
  "id",
  "component",
  "children",
  "child",
  "content",
  "trigger",
  "tabs",
  "action",
  "checks",
]);

// ─── 内部 Surface 状态 ──────────────────────────────────────────────────────

interface SurfaceEntry {
  surfaceId: SurfaceId;
  catalogId: string;
  lifecycle: SurfaceLifecycle;
  sendDataModel: boolean;
  /** 扁平组件 map（邻接表）。 */
  components: Map<ComponentId, Component>;
  /** 插入顺序，供 root 检测的确定性回退。 */
  order: ComponentId[];
  dataModel: DataModel;
  graph: DependencyGraph;
}

/** 待 actionResponse 写回的登记项。 */
interface PendingAction {
  surfaceId: SurfaceId;
  responsePath?: string;
}

// ─── 作用域工具 ─────────────────────────────────────────────────────────────

/** 用 Scope 的集合帧初始化一个 PathResolver（根帧 + 各集合帧）。 */
function resolverForScope(dataModel: DataModel, scope: Scope): PathResolver {
  const resolver = new PathResolver(dataModel);
  for (const frame of scope.frames) {
    resolver.enterCollection(frame.basePath, frame.index);
  }
  return resolver;
}

// ─── 值求值（DynamicValue，含依赖收集） ─────────────────────────────────────

interface EvalCtx {
  resolver: PathResolver;
  dispatcher: FunctionDispatcher;
  deps: Set<string>;
}

/** 求值任意（可能内嵌 DynamicValue 的）JSON 节点，收集依赖路径。 */
function evalValue(node: unknown, ctx: EvalCtx): Json | undefined {
  if (isDataBinding(node)) {
    ctx.deps.add(ctx.resolver.makeAbsolute(node.path));
    return ctx.resolver.resolve(node.path);
  }
  if (isFunctionCall(node)) {
    return evalCall(node.call, node.args, ctx);
  }
  if (Array.isArray(node)) {
    return node.map((item) => evalValue(item, ctx) ?? null);
  }
  if (isObject(node)) {
    const out: { [key: string]: Json } = {};
    for (const [k, v] of Object.entries(node)) {
      const r = evalValue(v, ctx);
      if (r !== undefined) out[k] = r;
    }
    return out;
  }
  return node as Json;
}

function evalCall(
  call: string,
  args: unknown,
  ctx: EvalCtx,
): Json | undefined {
  if (call === "@index") {
    const idx = ctx.resolver.currentIndex();
    return idx === undefined ? undefined : idx;
  }
  const argObj = isObject(args) ? args : {};

  if (call === "formatString") {
    const template = typeof argObj.template === "string" ? argObj.template : "";
    const bindingsRaw = isObject(argObj.bindings) ? argObj.bindings : {};
    const values: Record<string, Json> = {};
    for (const [k, v] of Object.entries(bindingsRaw)) {
      const r = evalValue(v, ctx);
      values[k] = r === undefined ? "" : r;
    }
    return formatString(template, values);
  }

  // 其余函数：先求值 args 中的动态值，再走调度器（client 边界）。
  const resolvedArgs: Record<string, Json> = {};
  for (const [k, v] of Object.entries(argObj)) {
    const r = evalValue(v, ctx);
    if (r !== undefined) resolvedArgs[k] = r;
  }
  try {
    return ctx.dispatcher.dispatch(call, resolvedArgs, "client");
  } catch (e) {
    if (e instanceof FunctionError) return undefined;
    throw e;
  }
}

// ─── 子引用形态 ─────────────────────────────────────────────────────────────

type ChildRefs =
  | { mode: "none" }
  | { mode: "single"; ids: ComponentId[] }
  | { mode: "list"; ids: ComponentId[] }
  | { mode: "template"; template: ComponentId; path: string };

function childRefsOf(component: Component): ChildRefs {
  const p = component as Record<string, unknown>;
  if (typeof p.child === "string") return { mode: "single", ids: [p.child] };
  if (typeof p.content === "string") {
    const ids = [p.content];
    if (typeof p.trigger === "string") ids.push(p.trigger);
    return { mode: "single", ids };
  }
  if (Array.isArray(p.tabs)) {
    const ids: ComponentId[] = [];
    for (const t of p.tabs) {
      if (isObject(t) && typeof t.child === "string") ids.push(t.child);
    }
    return { mode: "list", ids };
  }
  const children = p.children;
  if (isTemplateChildList(children)) {
    return { mode: "template", template: children.template, path: children.path };
  }
  if (Array.isArray(children)) {
    const ids = children.filter((c): c is string => typeof c === "string");
    return { mode: "list", ids };
  }
  return { mode: "none" };
}

/** 确定性的节点键：组件 id + 各集合帧索引，供依赖图登记（同一模板不同项互不干扰）。 */
function nodeKey(ref: NodeRef): string {
  if (ref.scope.frames.length === 0) return ref.componentId;
  const suffix = ref.scope.frames.map((f) => `${f.basePath}:${f.index}`).join("|");
  return `${ref.componentId}@${suffix}`;
}

// ─── SurfaceStore 实现 ──────────────────────────────────────────────────────

class Store implements SurfaceStore {
  private readonly surfaces = new Map<SurfaceId, SurfaceEntry>();
  private readonly dispatcher: FunctionDispatcher;
  private readonly listeners = new Set<() => void>();
  private readonly pending = new Map<string, PendingAction>();

  constructor(dispatcher: FunctionDispatcher) {
    this.dispatcher = dispatcher;
  }

  /** 客户端函数注册表（供上层扩展与 callFunction 边界校验）。 */
  get functions(): FunctionDispatcher {
    return this.dispatcher;
  }

  ingest(envelope: ServerEnvelope): void {
    if (typeof envelope !== "object" || envelope === null) return;
    if (envelope.version !== "v1.0") return;

    if (envelope.createSurface) {
      this.onCreateSurface(envelope.createSurface);
      return;
    }
    if (envelope.updateComponents) {
      const m = envelope.updateComponents;
      const s = this.surfaces.get(m.surfaceId);
      if (!s || s.lifecycle === "deleted") return;
      this.upsertComponents(s, m.components ?? []);
      this.notify();
      return;
    }
    if (envelope.updateDataModel) {
      const m = envelope.updateDataModel;
      const s = this.surfaces.get(m.surfaceId);
      if (!s || s.lifecycle === "deleted") return;
      // A2UI v1.0：省略 value 或显式 null 均删除该路径（仅非 null 值 upsert）。
      const hasValue = Object.prototype.hasOwnProperty.call(m, "value");
      if (hasValue && m.value !== null) {
        s.dataModel.applyPointer(m.path ?? "/", m.value as Json);
      } else {
        s.dataModel.deletePointer(m.path ?? "/");
      }
      this.notify();
      return;
    }
    if (envelope.deleteSurface) {
      const s = this.surfaces.get(envelope.deleteSurface.surfaceId);
      if (!s) return;
      s.lifecycle = "deleted";
      s.graph.reset();
      // 释放组件与渲染数据；entry 保留在 map 中供 getSurface 查询 lifecycle
      s.components.clear();
      s.order = [];
      this.notify();
      return;
    }
    if (envelope.actionResponse && typeof envelope.actionId === "string") {
      this.onActionResponse(envelope.actionId, envelope.actionResponse);
      return;
    }
    // callFunction / functionResponse 等：M1 不在此闭环处理（无回传通道）。
  }

  private onCreateSurface(
    m: NonNullable<ServerEnvelope["createSurface"]>,
  ): void {
    if (typeof m.surfaceId !== "string" || typeof m.catalogId !== "string") return;
    const entry: SurfaceEntry = {
      surfaceId: m.surfaceId,
      catalogId: m.catalogId,
      lifecycle: "active",
      sendDataModel: m.sendDataModel === true,
      components: new Map(),
      order: [],
      dataModel: new DataModel((m.dataModel as Json) ?? {}),
      graph: new DependencyGraph(),
    };
    this.upsertComponents(entry, m.components ?? []);
    this.surfaces.set(m.surfaceId, entry);
    this.notify();
  }

  private upsertComponents(s: SurfaceEntry, components: Component[]): void {
    for (const c of components) {
      if (!isObject(c) || typeof c.id !== "string" || typeof c.component !== "string") {
        continue;
      }
      if (!s.components.has(c.id)) s.order.push(c.id);
      s.components.set(c.id, c);
    }
  }

  private onActionResponse(
    actionId: string,
    resp: NonNullable<ServerEnvelope["actionResponse"]>,
  ): void {
    const pending = this.pending.get(actionId);
    this.pending.delete(actionId);
    if (pending && pending.responsePath !== undefined && resp.error === undefined) {
      const s = this.surfaces.get(pending.surfaceId);
      if (s && s.lifecycle !== "deleted") {
        s.dataModel.applyPointer(pending.responsePath, (resp.value as Json) ?? null);
        this.notify();
      }
    }
  }

  getSurfaceIds(): SurfaceId[] {
    // 生命周期在读取路径生效：已删除的 surface 不再被渲染层枚举
    return [...this.surfaces.values()]
      .filter((s) => s.lifecycle !== "deleted")
      .map((s) => s.surfaceId);
  }

  getSurface(surfaceId: SurfaceId): SurfaceSnapshot | undefined {
    const s = this.surfaces.get(surfaceId);
    if (!s) return undefined;
    const root = this.rootRef(s);
    return {
      surfaceId: s.surfaceId,
      catalogId: s.catalogId,
      lifecycle: s.lifecycle,
      root,
    };
  }

  getRootRef(surfaceId: SurfaceId): NodeRef | undefined {
    const s = this.surfaces.get(surfaceId);
    if (!s || s.lifecycle === "deleted") return undefined;
    return this.rootRef(s);
  }

  private rootRef(s: SurfaceEntry): NodeRef | undefined {
    const rootId = this.rootId(s);
    return rootId === undefined
      ? undefined
      : { componentId: rootId, scope: ROOT_SCOPE };
  }

  /** 约定名优先（`root` / `root_card`），否则取未被引用者，再退回插入顺序。 */
  private rootId(s: SurfaceEntry): ComponentId | undefined {
    if (s.components.size === 0) return undefined;
    if (s.components.has("root")) return "root";
    if (s.components.has("root_card")) return "root_card";
    const referenced = new Set<ComponentId>();
    for (const c of s.components.values()) {
      const refs = childRefsOf(c);
      if (refs.mode === "single" || refs.mode === "list") {
        for (const id of refs.ids) referenced.add(id);
      } else if (refs.mode === "template") {
        referenced.add(refs.template);
      }
    }
    return s.order.find((id) => !referenced.has(id));
  }

  resolveNode(surfaceId: SurfaceId, ref: NodeRef): ResolvedNode | undefined {
    const s = this.surfaces.get(surfaceId);
    if (!s || s.lifecycle === "deleted") return undefined;
    const component = s.components.get(ref.componentId);
    if (!component) {
      // 引用缺失：渐进式渲染占位符。
      return {
        id: ref.componentId,
        component: "Placeholder",
        props: {},
        children: [],
        placeholder: `missing component: ${ref.componentId}`,
      };
    }

    const resolver = resolverForScope(s.dataModel, ref.scope);
    const ctx: EvalCtx = { resolver, dispatcher: this.dispatcher, deps: new Set() };

    // 求值非结构性属性。
    const props: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(component)) {
      if (STRUCTURAL_KEYS.has(key)) continue;
      const r = evalValue(value, ctx);
      if (r !== undefined) props[key] = r;
    }

    // Tabs 的标签标题是结构性数据（child 引用由 children 抽取），但渲染层需要标题；
    // 单独透传，顺序与 children 一致。
    const rawTabs = (component as Record<string, unknown>).tabs;
    if (Array.isArray(rawTabs)) {
      props.tabs = rawTabs.map((t) =>
        isObject(t) ? { title: t.title } : t,
      );
    }

    const children = this.childRefs(component, resolver, ctx);

    const node: ResolvedNode = {
      id: ref.componentId,
      component: component.component,
      props,
      children,
    };

    // action（事件或本地函数调用）。
    const raw = component as Record<string, unknown>;
    if (isObject(raw.action)) node.action = raw.action as unknown as Action;

    // 输入组件双向绑定的绝对路径（供写回）。
    const bindingPath = this.bindingPathOf(component, resolver);
    if (bindingPath !== undefined) node.bindingPath = bindingPath;

    // checks 校验（最小集）。
    const { disabled, errors } = this.evalChecks(component, ctx);
    if (errors.length > 0) node.errors = errors;
    if (disabled) node.disabled = true;

    // 登记依赖，供响应性反查。
    s.graph.set(nodeKey(ref), ctx.deps);

    return node;
  }

  private childRefs(
    component: Component,
    resolver: PathResolver,
    ctx: EvalCtx,
  ): NodeRef[] {
    const refs = childRefsOf(component);
    switch (refs.mode) {
      case "none":
        return [];
      case "single":
      case "list":
        return refs.ids.map((id) => ({
          componentId: id,
          scope: cloneScope(resolver),
        }));
      case "template": {
        ctx.deps.add(resolver.makeAbsolute(refs.path));
        const arr = resolver.resolve(refs.path);
        if (!Array.isArray(arr)) return [];
        const absBase = resolver.makeAbsolute(refs.path);
        const baseFrames = cloneScope(resolver).frames;
        return arr.map((_, index) => {
          const frame: CollectionFrame = { basePath: absBase, index };
          return {
            componentId: refs.template,
            scope: { frames: [...baseFrames, frame] },
          };
        });
      }
    }
  }

  /** TextField 等输入组件绑定 `value` 的绝对 data 路径。 */
  private bindingPathOf(
    component: Component,
    resolver: PathResolver,
  ): string | undefined {
    const value = (component as Record<string, unknown>).value;
    if (isDataBinding(value)) return resolver.makeAbsolute(value.path);
    return undefined;
  }

  /** 求值 `checks`（内置 `required` 等）；任何失败 → disabled + errors。 */
  private evalChecks(
    component: Component,
    ctx: EvalCtx,
  ): { disabled: boolean; errors: CheckError[] } {
    const checks = (component as Record<string, unknown>).checks;
    if (!Array.isArray(checks)) return { disabled: false, errors: [] };
    const errors: CheckError[] = [];
    checks.forEach((check, checkIndex) => {
      if (!isObject(check)) return;
      const message =
        typeof check.message === "string" ? check.message : "check failed";
      const spec = check.check ?? check;
      const result = evalValue(spec, ctx);
      if (result === false) {
        errors.push({ message, componentId: component.id, checkIndex });
      }
    });
    return { disabled: errors.length > 0, errors };
  }

  getDataValue(surfaceId: SurfaceId, path: string): unknown {
    const s = this.surfaces.get(surfaceId);
    return s ? s.dataModel.resolvePointer(path) : undefined;
  }

  setDataValue(surfaceId: SurfaceId, path: string, value: unknown): void {
    const s = this.surfaces.get(surfaceId);
    if (!s || s.lifecycle === "deleted") return;
    s.dataModel.applyPointer(path, value as Json);
    this.notify();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify(): void {
    for (const l of this.listeners) l();
  }

  buildActionEnvelope(
    surfaceId: SurfaceId,
    action: Action,
    sourceComponentId?: ComponentId,
    scope: Scope = ROOT_SCOPE,
  ): ClientEnvelope {
    const entry = this.surfaces.get(surfaceId);
    // 已删除 surface 上的交互不再携带其状态（与 setDataValue 的守卫一致）
    const s = entry && entry.lifecycle !== "deleted" ? entry : undefined;
    // 函数调用型 action 无回传语义：返回一个空 action 信封（V 侧应本地执行）。
    if (!isEventAction(action)) {
      return { version: "v1.0" };
    }

    const message: ActionMessage = { name: action.name, surfaceId };
    if (sourceComponentId !== undefined) message.sourceComponentId = sourceComponentId;

    if (action.context && Object.keys(action.context).length > 0) {
      const resolver = s
        ? resolverForScope(s.dataModel, scope)
        : new PathResolver(new DataModel({}));
      const ctx: EvalCtx = { resolver, dispatcher: this.dispatcher, deps: new Set() };
      const context: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(action.context)) {
        const r = evalValue(v, ctx);
        context[k] = r === undefined ? null : r;
      }
      message.context = context;
    }
    if (action.wantResponse) message.wantResponse = true;
    if (action.responsePath !== undefined) message.responsePath = action.responsePath;
    if (action.actionId !== undefined) message.actionId = action.actionId;

    // 登记 actionId → responsePath，供后续 actionResponse 写回。
    if (message.wantResponse && message.actionId) {
      this.pending.set(message.actionId, {
        surfaceId,
        responsePath: message.responsePath,
      });
    }

    const envelope: ClientEnvelope = { version: "v1.0", action: message };
    if (s?.sendDataModel) {
      envelope.metadata = { surfaceId, dataModel: s.dataModel.value };
    }
    return envelope;
  }
}

/** 快照当前解析器的作用域（用于生成子 NodeRef 的 scope）。 */
function cloneScope(resolver: PathResolver): Scope {
  return { frames: resolver.frames() };
}

/**
 * 创建一个 {@link SurfaceStore}。可传入自定义 {@link FunctionDispatcher} 扩展客户端
 * 函数注册表；默认注册内置函数（`required` / `formatString`）。
 */
export function createSurfaceStore(
  dispatcher: FunctionDispatcher = new FunctionDispatcher(),
): SurfaceStore {
  return new Store(dispatcher);
}
