/**
 * Surface：一个独立 UI 区域的协议状态与解析引擎（纯 TS，无 React）。
 *
 * 职责：
 * - 组件森林：扁平 map + 按 id 引用建树；root 未到达前缓冲、缺失引用留占位（渐进式）。
 * - 生命周期状态机：`pending` → `active` → `deleted`。
 * - Data Model 读写 + 依赖图响应性：解析组件时登记依赖路径，数据变更时反查受影响
 *   组件并通知订阅者（本层不依赖 React）。
 * - 作用域解析：根作用域 + 集合作用域（ChildList template，`@index` 注入）。
 * - 交互回传：{@link buildActionMessage} 生成 ClientEnvelope，`sendDataModel` 时附带 model。
 */
import { DataModel } from "@/core/data-model";
import { PathResolver } from "@/core/path-resolver";
import { FunctionDispatcher } from "@/core/functions";
import { DependencyGraph } from "@/core/dependency-graph";
import {
  resolveComponentProps,
  resolveValue,
  type ResolveContext,
} from "@/core/resolve";
import {
  isTemplateChildList,
  isFunctionCallAction,
  type Component,
  type ComponentId,
  type Json,
  type ClientEnvelope,
  type ActionMessage,
  type EventActionSpec,
  type ActionSpec,
} from "@/core/types";

/** Surface 生命周期状态。 */
export type SurfaceState = "pending" | "active" | "deleted";

/** 组件的子引用形态（结构层，未展开 template）。 */
export type ChildRefs =
  | { mode: "none" }
  | { mode: "single"; ids: ComponentId[] }
  | { mode: "list"; ids: ComponentId[] }
  | { mode: "template"; template: ComponentId; path: string };

/** 某组件在给定作用域下解析后的视图数据。 */
export interface ResolvedComponent {
  id: ComponentId;
  type: string;
  props: Record<string, Json>;
  action?: ActionSpec;
  childRefs: ChildRefs;
  deps: Set<string>;
}

/** 展开后的渲染树节点（template 实例带索引实例 id 与 `@index`）。 */
export interface ResolvedNode {
  /** 实例 id：普通组件即组件 id；template 实例为 `${templateId}#${index}`。 */
  id: string;
  /** 原始组件 id（template 实例为模板 id）。 */
  componentId: ComponentId;
  type: string;
  props: Record<string, Json>;
  action?: ActionSpec;
  children: ResolvedNode[];
  /** 引用缺失（渐进式渲染占位）时为 true。 */
  placeholder?: boolean;
}

/** 一次交互回传：待发送的信封 + （可选）随附的 data model。 */
export interface ActionDispatch {
  envelope: ClientEnvelope;
  /** 仅当 `sendDataModel` 为真时存在（供 transport 以 metadata 附带）。 */
  dataModel?: Json;
}

/** 数据变更通知。 */
export interface ChangeNotification {
  /** 变更的绝对路径（根替换为 `"/"`）。 */
  path: string;
  /** 受影响的组件/节点 id 集合。 */
  affected: Set<string>;
}

type Listener = (n: ChangeNotification) => void;

const PLACEHOLDER_TYPE = "__placeholder__";

export class Surface {
  readonly surfaceId: string;
  readonly catalogId: string;
  readonly sendDataModel: boolean;

  private stateValue: SurfaceState = "pending";
  private readonly components = new Map<ComponentId, Component>();
  /** 插入顺序，用于 root 检测的确定性回退。 */
  private readonly order: ComponentId[] = [];
  private dataModel: DataModel;
  private readonly dispatcher: FunctionDispatcher;
  private readonly graph = new DependencyGraph();
  private readonly listeners = new Set<Listener>();
  private readonly componentListeners = new Map<string, Set<Listener>>();

  constructor(
    surfaceId: string,
    catalogId: string,
    options: {
      sendDataModel?: boolean;
      dataModel?: Json;
      dispatcher?: FunctionDispatcher;
    } = {},
  ) {
    this.surfaceId = surfaceId;
    this.catalogId = catalogId;
    this.sendDataModel = options.sendDataModel ?? false;
    this.dataModel = new DataModel(options.dataModel ?? {});
    this.dispatcher = options.dispatcher ?? new FunctionDispatcher();
  }

  // ---- 生命周期 ----

  get state(): SurfaceState {
    return this.stateValue;
  }

  /** 标记为 active（收到 createSurface 后）。 */
  activate(): void {
    if (this.stateValue === "pending") this.stateValue = "active";
  }

  /** 标记为 deleted 并清理响应性资源。 */
  markDeleted(): void {
    this.stateValue = "deleted";
    this.graph.reset();
    this.listeners.clear();
    this.componentListeners.clear();
  }

  // ---- 组件森林 ----

  /** 增量加入/更新组件（邻接表）。deleted 状态下忽略。 */
  upsertComponents(components: Component[]): void {
    if (this.stateValue === "deleted") return;
    for (const c of components) {
      if (!this.components.has(c.id)) this.order.push(c.id);
      this.components.set(c.id, c);
    }
  }

  /** 读取原始组件定义。 */
  getComponent(id: ComponentId): Component | undefined {
    return this.components.get(id);
  }

  /** 已加入的组件数量。 */
  get componentCount(): number {
    return this.components.size;
  }

  /**
   * 检测 root 组件 id：优先约定名 `root` / `root_card`，否则取唯一未被任何组件
   * 引用为子节点者，再退回插入顺序首个未被引用者。root 未到达返回 `undefined`。
   */
  getRootId(): ComponentId | undefined {
    if (this.components.size === 0) return undefined;
    if (this.components.has("root")) return "root";
    if (this.components.has("root_card")) return "root_card";
    const referenced = this.referencedIds();
    const roots = this.order.filter((id) => !referenced.has(id));
    return roots[0];
  }

  private referencedIds(): Set<ComponentId> {
    const set = new Set<ComponentId>();
    for (const c of this.components.values()) {
      const refs = childRefsOf(c);
      switch (refs.mode) {
        case "single":
        case "list":
          for (const id of refs.ids) set.add(id);
          break;
        case "template":
          set.add(refs.template);
          break;
        case "none":
          break;
      }
    }
    return set;
  }

  // ---- Data Model ----

  /** 整个 data model 的 JSON 值。 */
  getDataModel(): Json {
    return this.dataModel.value;
  }

  /** 读取某路径的值（安全，未命中返回 undefined）。 */
  getDataValue(path: string): Json | undefined {
    return this.dataModel.resolvePointer(path);
  }

  /**
   * 应用一次 updateDataModel（协议消息语义）。
   * @param hasValue 是否显式带了 value 字段（false = 删除，true = 设置含 null）。
   */
  applyDataModel(path: string | undefined, hasValue: boolean, value?: Json): void {
    if (this.stateValue === "deleted") return;
    const pointer = path ?? "/";
    const change = hasValue
      ? this.dataModel.applyPointer(pointer, value)
      : this.dataModel.deletePointer(pointer);
    this.notifyChange(change.path);
  }

  /**
   * 双向绑定写回：立即更新本地 model 并触发响应性（不发网络请求）。
   * 供 React 输入组件在用户交互时调用。
   */
  setDataValue(path: string, value: Json): void {
    if (this.stateValue === "deleted") return;
    const change = this.dataModel.applyPointer(path, value);
    this.notifyChange(change.path);
  }

  // ---- 订阅 / 通知 ----

  /** 订阅任意数据变更；返回取消订阅函数。 */
  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** 订阅某组件（或 template 实例 id）相关的变更；返回取消订阅函数。 */
  subscribeComponent(nodeId: string, listener: Listener): () => void {
    let set = this.componentListeners.get(nodeId);
    if (!set) {
      set = new Set();
      this.componentListeners.set(nodeId, set);
    }
    set.add(listener);
    return () => set!.delete(listener);
  }

  private notifyChange(path: string): void {
    const affected = this.graph.affectedBy(path);
    const notification: ChangeNotification = { path, affected };
    for (const l of this.listeners) l(notification);
    for (const id of affected) {
      const set = this.componentListeners.get(id);
      if (set) for (const l of set) l(notification);
    }
  }

  // ---- 解析 ----

  private newResolver(): PathResolver {
    return new PathResolver(this.dataModel);
  }

  /**
   * 解析单个组件为视图数据（根作用域），并登记其依赖路径到依赖图。
   * 缺失组件返回 `undefined`。
   */
  resolveComponent(id: ComponentId): ResolvedComponent | undefined {
    const resolver = this.newResolver();
    return this.resolveWith(id, resolver);
  }

  private resolveWith(
    id: ComponentId,
    resolver: PathResolver,
    nodeId: string = id,
  ): ResolvedComponent | undefined {
    const component = this.components.get(id);
    if (!component) return undefined;
    const { props, deps } = resolveComponentProps(
      component,
      resolver,
      this.dispatcher,
    );
    const action = component.properties.action as unknown as
      | ActionSpec
      | undefined;
    this.graph.set(nodeId, deps);
    return {
      id,
      type: component.component,
      props,
      action: action ?? undefined,
      childRefs: childRefsOf(component),
      deps,
    };
  }

  /**
   * 构建完整渲染树（从 root 展开，含 template 迭代与 `@index`）。
   * root 未到达返回 `undefined`；缺失引用产出占位节点。
   */
  getRenderTree(): ResolvedNode | undefined {
    const rootId = this.getRootId();
    if (rootId === undefined) return undefined;
    const resolver = this.newResolver();
    return this.buildNode(rootId, resolver, rootId);
  }

  private buildNode(
    id: ComponentId,
    resolver: PathResolver,
    nodeId: string,
  ): ResolvedNode {
    const component = this.components.get(id);
    if (!component) {
      return {
        id: nodeId,
        componentId: id,
        type: PLACEHOLDER_TYPE,
        props: {},
        children: [],
        placeholder: true,
      };
    }
    const resolved = this.resolveWith(id, resolver, nodeId)!;
    const children = this.buildChildren(resolved.childRefs, resolver, nodeId);
    return {
      id: nodeId,
      componentId: id,
      type: resolved.type,
      props: resolved.props,
      action: resolved.action,
      children,
    };
  }

  private buildChildren(
    refs: ChildRefs,
    resolver: PathResolver,
    _parentNodeId: string,
  ): ResolvedNode[] {
    switch (refs.mode) {
      case "none":
        return [];
      case "single":
      case "list":
        return refs.ids.map((childId) =>
          this.buildNode(childId, resolver, childId),
        );
      case "template": {
        const arr = resolver.resolve(refs.path);
        if (!Array.isArray(arr)) return [];
        const absBase = resolver.makeAbsolute(refs.path);
        return arr.map((_, index) => {
          const instanceId = `${refs.template}#${index}`;
          return resolver.withCollection(absBase, index, () =>
            this.buildNode(refs.template, resolver, instanceId),
          );
        });
      }
    }
  }

  // ---- 交互回传 ----

  /**
   * 为某组件的 event action 生成 ClientEnvelope（用于回传服务端）。
   * context 中的动态值在当前 data model 下解析。`sendDataModel` 为真时随附 model。
   * 组件不存在、无 action、或 action 为本地函数调用型时返回 `undefined`。
   */
  buildActionMessage(componentId: ComponentId): ActionDispatch | undefined {
    const component = this.components.get(componentId);
    if (!component) return undefined;
    const action = component.properties.action as unknown as
      | ActionSpec
      | undefined;
    if (!action || isFunctionCallAction(action)) return undefined;

    const spec = action as EventActionSpec;
    const resolver = this.newResolver();
    const ctx: ResolveContext = {
      resolver,
      dispatcher: this.dispatcher,
      deps: new Set(),
    };

    const message: ActionMessage = {
      name: spec.name,
      surfaceId: this.surfaceId,
      sourceComponentId: componentId,
    };
    if (spec.context && Object.keys(spec.context).length > 0) {
      const context: Record<string, Json> = {};
      for (const [k, v] of Object.entries(spec.context)) {
        const r = resolveValue(v as Json, ctx);
        context[k] = r === undefined ? null : r;
      }
      message.context = context;
    }
    if (spec.wantResponse) message.wantResponse = true;
    if (spec.responsePath !== undefined) message.responsePath = spec.responsePath;
    if (spec.actionId !== undefined) message.actionId = spec.actionId;

    const dispatch: ActionDispatch = {
      envelope: { version: "v1.0", action: message },
    };
    if (this.sendDataModel) dispatch.dataModel = this.dataModel.value;
    return dispatch;
  }
}

/** 从组件属性推断子引用形态（结构层）。 */
export function childRefsOf(component: Component): ChildRefs {
  const p = component.properties;
  // Card / Button：单子组件
  if (typeof p.child === "string") {
    return { mode: "single", ids: [p.child] };
  }
  // Modal：content (+ trigger)
  if (typeof p.content === "string") {
    const ids = [p.content];
    if (typeof p.trigger === "string") ids.push(p.trigger);
    return { mode: "single", ids };
  }
  // Tabs：tabs[].child
  if (Array.isArray(p.tabs)) {
    const ids: ComponentId[] = [];
    for (const t of p.tabs) {
      if (t && typeof t === "object" && !Array.isArray(t)) {
        const child = (t as Record<string, Json>).child;
        if (typeof child === "string") ids.push(child);
      }
    }
    return { mode: "list", ids };
  }
  // Row / Column / List：children（数组 or template）
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
