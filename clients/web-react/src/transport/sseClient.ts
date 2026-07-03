// 浏览器端 A2UI SSE 客户端（B2 传输层，与 wsClient 平级、可互换）。
//
// SSE 是单向的（服务端→客户端），所以收发拆成两个 URL：
// - `eventsUrl`：`EventSource` 订阅，服务端每个 SSE 帧是一条 `ServerEnvelope`；
// - `actionUrl`：交互产生的 `ClientEnvelope` 用 `fetch` POST 回传。
//
// 对接的 Agent Host（如 Salvo）只需提供 `GET eventsUrl`(SSE) 与 `POST actionUrl`
// 两个端点；协议格式与 wsClient 完全一致（同一份 `@/contracts` 类型）。

import type { ClientEnvelope, ServerEnvelope } from "@/contracts";

export type SseStatus = "connecting" | "open" | "closed";

export interface SseClientOptions {
  /** SSE 事件流地址（`GET`，每帧一条 ServerEnvelope）。 */
  eventsUrl: string;
  /** 动作回传地址（`POST` 一条 ClientEnvelope）。 */
  actionUrl: string;
  /** 收到一条服务端信封时回调（通常喂给 `SurfaceStore.ingest`）。 */
  onEnvelope: (envelope: ServerEnvelope) => void;
  /** 连接状态变化回调（用于 UI 状态提示）。 */
  onStatusChange?: (status: SseStatus) => void;
  /** 断线重连间隔，毫秒。设为 0 关闭自动重连。默认 1000。 */
  reconnectDelayMs?: number;
  /** 透传给回传 `fetch` 的额外选项（如鉴权 header、credentials）。 */
  fetchInit?: Omit<RequestInit, "method" | "body">;
}

export interface SseClient {
  /** 回传一条客户端信封到服务端（POST actionUrl）。 */
  send: (envelope: ClientEnvelope) => void;
  /** 主动关闭事件流并停止重连。 */
  close: () => void;
}

/**
 * 建立到 A2UI SSE Agent Host 的连接。
 *
 * @example
 * ```ts
 * const store = createSurfaceStore();
 * const client = createSseClient({
 *   eventsUrl: "/a2ui/events?surface=s1",
 *   actionUrl: "/a2ui/action",
 *   onEnvelope: (env) => store.ingest(env),
 * });
 * // 交互回传： <A2UIProvider onClientMessage={client.send} .../>
 * // 卸载时： client.close()
 * ```
 */
export function createSseClient(opts: SseClientOptions): SseClient {
  const {
    eventsUrl,
    actionUrl,
    onEnvelope,
    onStatusChange,
    reconnectDelayMs = 1000,
    fetchInit,
  } = opts;

  let source: EventSource | null = null;
  let closedByUser = false;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const connect = (): void => {
    onStatusChange?.("connecting");
    source = new EventSource(eventsUrl);

    source.onopen = () => onStatusChange?.("open");

    source.onmessage = (ev: MessageEvent) => {
      if (typeof ev.data !== "string") return;
      let envelope: ServerEnvelope;
      try {
        envelope = JSON.parse(ev.data) as ServerEnvelope;
      } catch {
        return; // 丢弃无法解析的帧
      }
      onEnvelope(envelope);
    };

    source.onerror = () => {
      onStatusChange?.("closed");
      // EventSource 会自行重连，但为与 wsClient 行为一致并可控，
      // 我们主动关闭后按配置重连（reconnectDelayMs<=0 时不重连）。
      source?.close();
      if (!closedByUser && reconnectDelayMs > 0) {
        reconnectTimer = setTimeout(connect, reconnectDelayMs);
      }
    };
  };

  connect();

  return {
    send: (envelope: ClientEnvelope) => {
      void fetch(actionUrl, {
        ...fetchInit,
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(fetchInit?.headers ?? {}),
        },
        body: JSON.stringify(envelope),
      }).catch(() => {
        // 回传失败不影响渲染；由上层按需重试。
      });
    },
    close: () => {
      closedByUser = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      source?.close();
      onStatusChange?.("closed");
    },
  };
}
