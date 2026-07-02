// 浏览器端 A2UI WebSocket 客户端（B2 传输层，M1.5 集成胶水）。
//
// 连接 Agent 侧 WS 服务端（如 Rust `serve_demo` 的 ws://127.0.0.1:8765），
// 把收到的每条 `ServerEnvelope` 交给回调（通常是 `store.ingest`），
// 并提供 `send` 把交互产生的 `ClientEnvelope` 回传服务端。含简单自动重连。

import type { ClientEnvelope, ServerEnvelope } from "@/contracts";

export type WsStatus = "connecting" | "open" | "closed";

export interface WsClientOptions {
  /** WS 服务端地址，如 `ws://127.0.0.1:8765`。 */
  url: string;
  /** 收到一条服务端信封时回调（通常喂给 `SurfaceStore.ingest`）。 */
  onEnvelope: (envelope: ServerEnvelope) => void;
  /** 连接状态变化回调（用于 UI 状态提示）。 */
  onStatusChange?: (status: WsStatus) => void;
  /** 断线重连间隔，毫秒。设为 0 关闭自动重连。默认 1000。 */
  reconnectDelayMs?: number;
}

export interface WsClient {
  /** 回传一条客户端信封到服务端（连接未就绪时静默丢弃）。 */
  send: (envelope: ClientEnvelope) => void;
  /** 主动关闭连接并停止重连。 */
  close: () => void;
}

/**
 * 建立到 A2UI WS 服务端的连接。
 *
 * @example
 * ```ts
 * const store = createSurfaceStore();
 * const client = createWsClient({
 *   url: "ws://127.0.0.1:8765",
 *   onEnvelope: (env) => store.ingest(env),
 * });
 * // ...交互时： client.send(clientEnvelope)
 * // 卸载时： client.close()
 * ```
 */
export function createWsClient(opts: WsClientOptions): WsClient {
  const { url, onEnvelope, onStatusChange, reconnectDelayMs = 1000 } = opts;
  let ws: WebSocket | null = null;
  let closedByUser = false;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const connect = (): void => {
    onStatusChange?.("connecting");
    ws = new WebSocket(url);

    ws.onopen = () => onStatusChange?.("open");

    ws.onmessage = (ev: MessageEvent) => {
      if (typeof ev.data !== "string") return;
      let envelope: ServerEnvelope;
      try {
        envelope = JSON.parse(ev.data) as ServerEnvelope;
      } catch {
        return; // 丢弃无法解析的帧
      }
      onEnvelope(envelope);
    };

    ws.onclose = () => {
      onStatusChange?.("closed");
      if (!closedByUser && reconnectDelayMs > 0) {
        reconnectTimer = setTimeout(connect, reconnectDelayMs);
      }
    };

    ws.onerror = () => ws?.close();
  };

  connect();

  return {
    send: (envelope: ClientEnvelope) => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(envelope));
      }
    },
    close: () => {
      closedByUser = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      ws?.close();
    },
  };
}
