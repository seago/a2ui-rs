/**
 * 路径解析引擎：根作用域 + 集合作用域（ChildList template）。
 *
 * 对齐 Rust `a2ui-renderer` 的 `PathResolver`：
 * - 绝对路径 `"/…"` 直接解析。
 * - `"@index"` 返回当前集合作用域的索引。
 * - 相对路径在集合作用域下解析为 `"/base/index/relative"`；根作用域下为 `"/relative"`。
 *
 * 解析只读，不修改 Data Model。写回（双向绑定）由上层调用
 * {@link "@/core/data-model".DataModel.applyPointer} 完成。
 */
import type { DataModel } from "@/core/data-model";
import type { Json } from "@/core/json-pointer";

/** 作用域帧：根或集合（数组迭代中的当前项）。 */
export type Scope =
  | { kind: "root" }
  | { kind: "collection"; basePath: string; index: number };

export class PathResolver {
  private readonly dataModel: DataModel;
  private scopeStack: Scope[] = [{ kind: "root" }];

  constructor(dataModel: DataModel) {
    this.dataModel = dataModel;
  }

  /** 解析路径（绝对 / `@index` / 相对），未命中返回 `undefined`。 */
  resolve(path: string): Json | undefined {
    if (path === "@index") {
      const idx = this.currentIndex();
      return idx === undefined ? undefined : idx;
    }
    return this.dataModel.resolvePointer(this.makeAbsolute(path));
  }

  /**
   * 将任意路径规范化为绝对 JSON Pointer（用于依赖登记）。
   * - `"/abs"` 原样返回。
   * - 相对路径：集合作用域 → `"/base/index/rel"`；根作用域 → `"/rel"`。
   * - `"@index"` 无对应数据路径，原样返回。
   */
  makeAbsolute(path: string): string {
    if (path.startsWith("/") || path === "@index") return path;
    const scope = this.currentScope();
    if (scope.kind === "collection") {
      return `${scope.basePath}/${scope.index}/${path}`;
    }
    return `/${path}`;
  }

  /** 当前集合作用域索引；根作用域返回 `undefined`。 */
  currentIndex(): number | undefined {
    const scope = this.currentScope();
    return scope.kind === "collection" ? scope.index : undefined;
  }

  /**
   * 当前作用域的集合帧快照（根帧除外），按从外到内的顺序。
   * 供 SurfaceStore 从解析器状态构造子 NodeRef 的 `Scope`。
   */
  frames(): { basePath: string; index: number }[] {
    const out: { basePath: string; index: number }[] = [];
    for (const s of this.scopeStack) {
      if (s.kind === "collection") out.push({ basePath: s.basePath, index: s.index });
    }
    return out;
  }

  /** 进入集合作用域（数组路径 + 当前项索引）。 */
  enterCollection(basePath: string, index: number): void {
    this.scopeStack.push({ kind: "collection", basePath, index });
  }

  /** 退出集合作用域（根帧不可弹出）。 */
  exitCollection(): void {
    if (this.scopeStack.length > 1) this.scopeStack.pop();
  }

  /** 在给定作用域内执行回调，结束后自动恢复（异常安全）。 */
  withCollection<T>(basePath: string, index: number, fn: () => T): T {
    this.enterCollection(basePath, index);
    try {
      return fn();
    } finally {
      this.exitCollection();
    }
  }

  private currentScope(): Scope {
    return this.scopeStack[this.scopeStack.length - 1];
  }
}
