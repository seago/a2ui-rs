import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ClientEnvelope, ServerEnvelope } from "@/contracts";
import { createSseClient } from "./sseClient";

// ─── 假 EventSource（SSE 收流）──────────────────────────────────────────────
class FakeEventSource {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  static instances: FakeEventSource[] = [];
  static last(): FakeEventSource {
    return FakeEventSource.instances[FakeEventSource.instances.length - 1];
  }

  url: string;
  readyState = FakeEventSource.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }
  close() {
    this.closed = true;
    this.readyState = FakeEventSource.CLOSED;
  }
  // 测试辅助
  open() {
    this.readyState = FakeEventSource.OPEN;
    this.onopen?.();
  }
  emit(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
  fail() {
    this.onerror?.();
  }
}

const EVENTS_URL = "http://host/a2ui/events?surface=s1";
const ACTION_URL = "http://host/a2ui/action";

const DEMO_ENVELOPE: ServerEnvelope = {
  version: "v1.0",
  createSurface: { surfaceId: "s1", catalogId: "basic", components: [] },
};

const ACTION_ENVELOPE: ClientEnvelope = {
  version: "v1.0",
  action: {
    name: "submit",
    surfaceId: "s1",
    sourceComponentId: "submit_btn",
    timestamp: "2026-07-07T00:00:00Z",
  },
};

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  fetchMock = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("createSseClient", () => {
  it("连到 events URL 并在 open 时报告状态", () => {
    const statuses: string[] = [];
    createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: () => {},
      onStatusChange: (s) => statuses.push(s),
    });
    expect(FakeEventSource.last().url).toBe(EVENTS_URL);
    expect(statuses).toContain("connecting");
    FakeEventSource.last().open();
    expect(statuses).toContain("open");
  });

  it("收到 SSE 帧解析为 ServerEnvelope 交给 onEnvelope", () => {
    const got: ServerEnvelope[] = [];
    createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: (e) => got.push(e),
    });
    FakeEventSource.last().open();
    FakeEventSource.last().emit(DEMO_ENVELOPE);
    expect(got).toHaveLength(1);
    expect(got[0].createSurface?.surfaceId).toBe("s1");
  });

  it("无法解析的帧被丢弃，不抛错", () => {
    const got: ServerEnvelope[] = [];
    createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: (e) => got.push(e),
    });
    FakeEventSource.last().open();
    FakeEventSource.last().onmessage?.({ data: "{not json" });
    expect(got).toHaveLength(0);
  });

  it("send 把 ClientEnvelope POST 到 actionUrl", () => {
    const client = createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: () => {},
    });
    client.send(ACTION_ENVELOPE);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe(ACTION_URL);
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual(ACTION_ENVELOPE);
    expect(init.headers["Content-Type"]).toContain("application/json");
  });

  it("close 关闭 EventSource 并停止重连", () => {
    const client = createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: () => {},
    });
    const es = FakeEventSource.last();
    client.close();
    expect(es.closed).toBe(true);
  });

  it("出错时报告 closed 状态并在 reconnectDelayMs>0 时重连", () => {
    vi.useFakeTimers();
    const statuses: string[] = [];
    createSseClient({
      eventsUrl: EVENTS_URL,
      actionUrl: ACTION_URL,
      onEnvelope: () => {},
      onStatusChange: (s) => statuses.push(s),
      reconnectDelayMs: 1000,
    });
    const first = FakeEventSource.last();
    first.fail();
    expect(statuses).toContain("closed");
    // 重连:定时器触发后新建一个 EventSource
    vi.advanceTimersByTime(1000);
    expect(FakeEventSource.instances.length).toBe(2);
    vi.useRealTimers();
  });
});
