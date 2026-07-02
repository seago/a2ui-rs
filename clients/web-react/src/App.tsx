import { useEffect, useMemo, useRef, useState } from "react";

import type { ClientEnvelope } from "@/contracts";
import { createSurfaceStore } from "@/core";
import { A2UIProvider, Surface, useA2UIContext } from "@/react";
import { shadcnKit } from "@/kits/shadcn";
import {
  createWsClient,
  type WsClient,
  type WsStatus,
} from "@/transport/wsClient";

const WS_URL = "ws://127.0.0.1:8765";

/** 渲染 store 中当前所有 Surface；空时提示等待。消费 context，随 store 变更重渲染。 */
function SurfaceList() {
  const { store } = useA2UIContext();
  const ids = store.getSurfaceIds();
  if (ids.length === 0) {
    return (
      <p className="text-muted-foreground" role="status">
        等待服务端推送 Surface…
      </p>
    );
  }
  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      {ids.map((id) => (
        <Surface key={id} surfaceId={id} />
      ))}
    </div>
  );
}

const STATUS_LABEL: Record<WsStatus, string> = {
  connecting: "连接中…",
  open: "已连接",
  closed: "已断开",
};

/**
 * B2 交互式 Web 渲染器入口。
 *
 * 建一个协议核心 store，连上 WS 服务端把消息喂进去，用 shadcn kit 渲染，
 * 并把交互产生的 ClientEnvelope 回传服务端 —— 完整交互闭环。
 */
export default function App() {
  const store = useMemo(() => createSurfaceStore(), []);
  const clientRef = useRef<WsClient | null>(null);
  const [status, setStatus] = useState<WsStatus>("connecting");

  useEffect(() => {
    const client = createWsClient({
      url: WS_URL,
      onEnvelope: (env) => store.ingest(env),
      onStatusChange: setStatus,
    });
    clientRef.current = client;
    return () => client.close();
  }, [store]);

  const onClientMessage = (env: ClientEnvelope) => clientRef.current?.send(env);

  return (
    <main className="min-h-svh flex flex-col items-center gap-6 p-8">
      <header className="flex flex-col items-center gap-1">
        <h1 className="text-2xl font-bold">A2UI Web Renderer</h1>
        <p className="text-sm text-muted-foreground">
          shadcn kit · {STATUS_LABEL[status]}
        </p>
      </header>
      <A2UIProvider
        store={store}
        kit={shadcnKit}
        onClientMessage={onClientMessage}
      >
        <SurfaceList />
      </A2UIProvider>
    </main>
  );
}
