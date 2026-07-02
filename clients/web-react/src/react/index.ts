/**
 * A2UI B2 渲染核心（轨道 V，React，组件库无关）。
 *
 * 遍历协议组件树，将每个节点映射到当前 {@link ComponentKit} 渲染，并接线交互回传：
 * - 依赖 C 的 `SurfaceStore` 与 K 的 `ComponentKit`（运行期注入，仅依赖 `@/contracts` 类型）。
 * - `A2UIProvider` 用 `useSyncExternalStore` 订阅 store 变更并驱动重渲染。
 * - `Surface` 取根节点递归渲染；tree-walker 按 `component` 分发到 kit 组件。
 *
 * @example
 * ```tsx
 * import { A2UIProvider, Surface } from "@/react";
 *
 * <A2UIProvider store={store} kit={kit} onClientMessage={send}>
 *   <Surface surfaceId="s1" />
 * </A2UIProvider>
 * ```
 */

export { A2UIProvider, useA2UIContext } from "./context";
export type { A2UIContextValue, A2UIProviderProps } from "./context";

export { Surface } from "./Surface";
export type { SurfaceProps } from "./Surface";

export { RenderNode, useRenderNode } from "./renderNode";
