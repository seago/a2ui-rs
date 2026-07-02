// A2UI React context —— 持有运行期注入的 SurfaceStore / ComponentKit / 回传回调。
//
// 只依赖 `@/contracts` 类型；真实实现（C 的 store、K 的 kit）在运行期注入。

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import type { ReactNode } from "react";

import type { ClientEnvelope, ComponentKit, SurfaceStore } from "@/contracts";

/** A2UI context 的载荷。 */
export interface A2UIContextValue {
  /** 协议核心 store（C 提供真实实现，测试用 mock）。 */
  store: SurfaceStore;
  /** 组件库实现（K 提供，测试用 mock）。 */
  kit: ComponentKit;
  /** 交互回传：由 Event 型 action 生成的信封经此送出。 */
  onClientMessage?: (env: ClientEnvelope) => void;
}

const A2UIContext = createContext<A2UIContextValue | null>(null);

/**
 * 读取 A2UI context。必须在 {@link A2UIProvider} 内使用。
 *
 * @example
 * ```tsx
 * const { store, kit } = useA2UIContext();
 * ```
 */
export function useA2UIContext(): A2UIContextValue {
  const ctx = useContext(A2UIContext);
  if (ctx === null) {
    throw new Error("useA2UIContext 必须在 <A2UIProvider> 内使用");
  }
  return ctx;
}

/** {@link A2UIProvider} 的 props。 */
export interface A2UIProviderProps extends A2UIContextValue {
  children?: ReactNode;
}

/**
 * 用 `useSyncExternalStore` 订阅 `store.subscribe`，store 变更时刷新 context
 * 值，从而驱动订阅方（Surface / 各节点）重渲染。
 *
 * @example
 * ```tsx
 * <A2UIProvider store={store} kit={kit} onClientMessage={send}>
 *   <Surface surfaceId="s1" />
 * </A2UIProvider>
 * ```
 */
export function A2UIProvider({
  store,
  kit,
  onClientMessage,
  children,
}: A2UIProviderProps): ReactNode {
  const version = useStoreVersion(store);

  // version 参与依赖：每次 store 通知都会产出新的 context 值，令消费方重渲染。
  const value = useMemo<A2UIContextValue>(
    () => ({ store, kit, onClientMessage }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [store, kit, onClientMessage, version],
  );

  return <A2UIContext.Provider value={value}>{children}</A2UIContext.Provider>;
}

/**
 * 桥接非 React 的 SurfaceStore：每次 `subscribe` 回调递增内部版本号，
 * `useSyncExternalStore` 据此触发重渲染。store 未提供快照版本，故本地维护。
 */
function useStoreVersion(store: SurfaceStore): number {
  const versionRef = useRef(0);

  const subscribe = useCallback(
    (onStoreChange: () => void) =>
      store.subscribe(() => {
        versionRef.current += 1;
        onStoreChange();
      }),
    [store],
  );

  const getSnapshot = useCallback(() => versionRef.current, []);

  return useSyncExternalStore(subscribe, getSnapshot);
}
