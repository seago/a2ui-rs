import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import App from "@/App";

/**
 * 端到端集成测试（M1 活骨架）：用 FakeWebSocket 模拟服务端，
 * 驱动 store → 渲染核心(V) → shadcn kit(K) → action 回传 的完整闭环。
 */

class FakeWebSocket {
  static readonly OPEN = 1;
  static instances: FakeWebSocket[] = [];
  static last(): FakeWebSocket {
    return FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
  }

  url: string;
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  sent: string[] = [];

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  send(data: string) {
    this.sent.push(data);
  }
  close() {
    this.readyState = 3;
    this.onclose?.();
  }

  // 测试辅助
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  emit(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
}

const DEMO_ENVELOPE = {
  version: "v1.0",
  createSurface: {
    surfaceId: "s1",
    catalogId: "basic",
    sendDataModel: true,
    dataModel: { form: { name: "" } },
    components: [
      { id: "root_card", component: "Card", child: "form_col" },
      {
        id: "form_col",
        component: "Column",
        children: ["title_text", "name_field", "submit_btn"],
      },
      { id: "title_text", component: "Text", text: "请输入你的名字" },
      {
        id: "name_field",
        component: "TextField",
        value: { path: "/form/name" },
        label: "姓名",
        variant: "shortText",
      },
      {
        id: "submit_btn",
        component: "Button",
        variant: "primary",
        child: "submit_label",
        action: {
          event: {
            name: "submit",
            wantResponse: true,
            actionId: "submit-1",
            responsePath: "/result",
          },
        },
      },
      { id: "submit_label", component: "Text", text: "提交" },
    ],
  },
};

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("App 端到端集成（M1 活骨架）", () => {
  it("初始展示等待态并建立 WS 连接", () => {
    render(<App />);
    expect(screen.getByText(/等待服务端推送/)).toBeInTheDocument();
    expect(FakeWebSocket.last().url).toBe("ws://127.0.0.1:8765");
  });

  it("收到 createSurface 后用 shadcn kit 渲染出表单", async () => {
    render(<App />);
    const sock = FakeWebSocket.last();
    act(() => {
      sock.open();
      sock.emit(DEMO_ENVELOPE);
    });

    expect(await screen.findByText("请输入你的名字")).toBeInTheDocument();
    expect(screen.getByLabelText("姓名")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "提交" }),
    ).toBeInTheDocument();
  });

  it("输入 + 点击提交，把 action 信封回传服务端", async () => {
    const user = userEvent.setup();
    render(<App />);
    const sock = FakeWebSocket.last();
    act(() => {
      sock.open();
      sock.emit(DEMO_ENVELOPE);
    });

    const field = await screen.findByLabelText("姓名");
    await user.type(field, "张三");
    await user.click(screen.getByRole("button", { name: "提交" }));

    expect(sock.sent.length).toBeGreaterThanOrEqual(1);
    const envelopes = sock.sent.map((s) => JSON.parse(s));
    const action = envelopes.find((e) => e.action)?.action;
    expect(action?.name).toBe("submit");
    // 规范必填字段与本地语义字段（responsePath 不上线路）
    expect(action?.sourceComponentId).toBe("submit_btn");
    expect(action?.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
    expect(action).not.toHaveProperty("responsePath");
  });
});
