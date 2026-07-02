/**
 * 依赖图：登记「节点 → 其依赖的 Data Model 绝对路径集合」，并在数据变更时
 * 反查受影响节点，实现声明式响应性（只重渲染依赖了变更路径的组件）。
 *
 * 节点键通常是组件 id；template 实例用带索引的实例 id（如 `item#0`）。
 * 路径匹配是双向前缀感知：变更 `/form` 影响依赖 `/form/name` 的节点；变更
 * `/form/name` 也影响依赖 `/form` 的节点；根变更 `"/"` 影响所有节点。
 */
export class DependencyGraph {
  private deps = new Map<string, Set<string>>();

  /** 覆盖登记某节点的依赖路径集合（空集合等价于清除）。 */
  set(nodeId: string, paths: Set<string>): void {
    if (paths.size === 0) {
      this.deps.delete(nodeId);
    } else {
      this.deps.set(nodeId, new Set(paths));
    }
  }

  /** 清除某节点的依赖登记。 */
  clear(nodeId: string): void {
    this.deps.delete(nodeId);
  }

  /** 清空全部登记（如 deleteSurface / 根替换后重建）。 */
  reset(): void {
    this.deps.clear();
  }

  /** 某节点当前登记的依赖路径（只读快照）。 */
  dependenciesOf(nodeId: string): ReadonlySet<string> {
    return this.deps.get(nodeId) ?? new Set();
  }

  /** 反查受某路径变更影响的所有节点 id。 */
  affectedBy(changedPath: string): Set<string> {
    const affected = new Set<string>();
    const isRoot = changedPath === "/" || changedPath === "";
    for (const [nodeId, paths] of this.deps) {
      if (isRoot) {
        affected.add(nodeId);
        continue;
      }
      for (const p of paths) {
        if (pathsOverlap(p, changedPath)) {
          affected.add(nodeId);
          break;
        }
      }
    }
    return affected;
  }
}

/** 两条绝对路径是否存在祖先/后代/相等关系（前缀感知，按段边界）。 */
export function pathsOverlap(a: string, b: string): boolean {
  if (a === b) return true;
  return a.startsWith(`${b}/`) || b.startsWith(`${a}/`);
}
