// Surface：取某 surface 的根节点引用并递归渲染；root 未到达时渲染空。

import type { ReactNode } from "react";

import type { SurfaceId } from "@/contracts";

import { useA2UIContext } from "./context";
import { RenderNode } from "./renderNode";

/** {@link Surface} 的 props。 */
export interface SurfaceProps {
  /** 要渲染的 surface。 */
  surfaceId: SurfaceId;
}

/**
 * 渲染单个 Surface：`store.getRootRef` 取根引用后递归渲染整棵组件树；
 * root 尚未到达（如 createSurface 前）时渲染空。
 *
 * @example
 * ```tsx
 * <A2UIProvider store={store} kit={kit}>
 *   <Surface surfaceId="s1" />
 * </A2UIProvider>
 * ```
 */
export function Surface({ surfaceId }: SurfaceProps): ReactNode {
  const { store } = useA2UIContext();
  const root = store.getRootRef(surfaceId);
  if (!root) return null;
  return <RenderNode surfaceId={surfaceId} nodeRef={root} />;
}
