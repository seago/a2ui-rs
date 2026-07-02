// TDD 测试：B2 渲染核心（轨道 V）。
//
// 依赖尚未就绪的 C（SurfaceStore）与 K（ComponentKit），此处用按契约实现的
// mock 顶替。只 import `@/contracts` 的类型，不 import `@/core` / `@/kits` 实现。

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { FC } from "react";

import type {
  Action,
  ClientEnvelope,
  ComponentId,
  ComponentKit,
  NodeRef,
  ResolvedNode,
  Scope,
  SurfaceId,
  SurfaceSnapshot,
  SurfaceStore,
} from "@/contracts";
import { ROOT_SCOPE } from "@/contracts";

import { A2UIProvider, Surface } from "./index";

// ─── mock ComponentKit：每组件渲染带 data-testid 的极简 DOM ──────────────────

function makeMockKit(): ComponentKit {
  const Text: FC<{ text: string; variant: string }> = ({ text, variant }) => (
    <span data-testid="kit-text" data-variant={variant}>
      {text}
    </span>
  );
  const Button: ComponentKit["Button"] = ({ label, variant, disabled, onAction }) => (
    <button
      data-testid="kit-button"
      data-variant={variant}
      disabled={disabled}
      onClick={onAction}
    >
      {label}
    </button>
  );
  const TextField: ComponentKit["TextField"] = ({
    value,
    onChange,
    label,
    placeholder,
    variant,
    disabled,
    errors,
  }) => (
    <label data-testid="kit-textfield-wrap" data-variant={variant}>
      {label}
      <input
        data-testid="kit-textfield"
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
      {errors.length > 0 ? <span data-testid="kit-errors">{errors.length}</span> : null}
    </label>
  );
  const Card: ComponentKit["Card"] = ({ children }) => (
    <div data-testid="kit-card">{children}</div>
  );
  const Column: ComponentKit["Column"] = ({ children }) => (
    <div data-testid="kit-column">{children}</div>
  );
  const Row: ComponentKit["Row"] = ({ children }) => (
    <div data-testid="kit-row">{children}</div>
  );
  const Placeholder: ComponentKit["Placeholder"] = ({ reason }) => (
    <div data-testid="kit-placeholder">{reason}</div>
  );
  return {
    Text: Text as ComponentKit["Text"],
    Button,
    TextField,
    Card,
    Column,
    Row,
    Placeholder,
  };
}

// ─── mock SurfaceStore：预置 ResolvedNode 树 + 记录写回 / 信封调用 ───────────

interface MockStore extends SurfaceStore {
  setCalls: Array<{ path: string; value: unknown }>;
  envelopeCalls: Array<{ action: Action; source?: ComponentId; scope?: Scope }>;
}

function makeMockStore(opts: {
  nodes: Record<ComponentId, ResolvedNode>;
  rootId?: ComponentId;
  data?: Record<string, unknown>;
}): MockStore {
  const data: Record<string, unknown> = { ...(opts.data ?? {}) };
  const setCalls: MockStore["setCalls"] = [];
  const envelopeCalls: MockStore["envelopeCalls"] = [];
  const SID: SurfaceId = "s1";

  const store: MockStore = {
    setCalls,
    envelopeCalls,
    ingest: () => {},
    getSurfaceIds: () => [SID],
    getSurface: (surfaceId): SurfaceSnapshot | undefined =>
      surfaceId === SID
        ? { surfaceId: SID, catalogId: "basic", lifecycle: "active" }
        : undefined,
    getRootRef: (surfaceId): NodeRef | undefined =>
      surfaceId === SID && opts.rootId
        ? { componentId: opts.rootId, scope: ROOT_SCOPE }
        : undefined,
    resolveNode: (_surfaceId, ref): ResolvedNode | undefined => opts.nodes[ref.componentId],
    getDataValue: (_surfaceId, path) => data[path],
    setDataValue: (_surfaceId, path, value) => {
      data[path] = value;
      setCalls.push({ path, value });
    },
    subscribe: () => () => {},
    buildActionEnvelope: (surfaceId, action, source, scope): ClientEnvelope => {
      envelopeCalls.push({ action, source, scope });
      return {
        version: "v1.0",
        action: {
          name: "name" in action ? action.name : "fn",
          surfaceId,
          sourceComponentId: source,
        },
      };
    },
  };
  return store;
}

// 复用的一棵树：card1(Card) → card2(Card) → [text1, field1, button1]
function buildTree(action: Action): Record<ComponentId, ResolvedNode> {
  return {
    card1: { id: "card1", component: "Card", props: {}, children: [ref("card2")] },
    card2: {
      id: "card2",
      component: "Card",
      props: {},
      children: [ref("text1"), ref("field1"), ref("button1")],
    },
    text1: {
      id: "text1",
      component: "Text",
      props: { text: "Hello", variant: "body" },
      children: [],
    },
    field1: {
      id: "field1",
      component: "TextField",
      props: { label: "Name", placeholder: "your name", variant: "shortText" },
      children: [],
      bindingPath: "/name",
    },
    button1: {
      id: "button1",
      component: "Button",
      props: { label: "Go", variant: "primary" },
      children: [],
      action,
    },
  };
}

function ref(id: ComponentId): NodeRef {
  return { componentId: id, scope: ROOT_SCOPE };
}

const EVENT_ACTION: Action = { name: "submit", context: { foo: "bar" } };

function renderSurface(store: MockStore, onClientMessage = vi.fn()) {
  render(
    <A2UIProvider store={store} kit={makeMockKit()} onClientMessage={onClientMessage}>
      <Surface surfaceId="s1" />
    </A2UIProvider>,
  );
  return { onClientMessage };
}

// ─── 测试 ───────────────────────────────────────────────────────────────────

describe("B2 渲染核心", () => {
  it("递归渲染 Card→Card→[Text, TextField, Button]", () => {
    const store = makeMockStore({
      nodes: buildTree(EVENT_ACTION),
      rootId: "card1",
      data: { "/name": "Ada" },
    });
    renderSurface(store);

    // 两层 Card 都渲染
    expect(screen.getAllByTestId("kit-card")).toHaveLength(2);
    // 叶子组件
    expect(screen.getByTestId("kit-text")).toHaveTextContent("Hello");
    expect(screen.getByTestId("kit-text")).toHaveAttribute("data-variant", "body");
    expect(screen.getByTestId("kit-textfield")).toBeInTheDocument();
    expect(screen.getByTestId("kit-button")).toHaveTextContent("Go");
  });

  it("TextField 的 value 取自 store.getDataValue(bindingPath)", () => {
    const store = makeMockStore({
      nodes: buildTree(EVENT_ACTION),
      rootId: "card1",
      data: { "/name": "Ada" },
    });
    renderSurface(store);
    expect(screen.getByTestId("kit-textfield")).toHaveValue("Ada");
  });

  it("TextField 输入触发 store.setDataValue(bindingPath, value)", async () => {
    const user = userEvent.setup();
    const store = makeMockStore({
      nodes: buildTree(EVENT_ACTION),
      rootId: "card1",
      data: { "/name": "" },
    });
    renderSurface(store);

    await user.type(screen.getByTestId("kit-textfield"), "X");
    expect(store.setCalls).toContainEqual({ path: "/name", value: "X" });
  });

  it("点击 Event 型 Button：调用 buildActionEnvelope 且 onClientMessage 收到信封", async () => {
    const user = userEvent.setup();
    const store = makeMockStore({
      nodes: buildTree(EVENT_ACTION),
      rootId: "card1",
    });
    const { onClientMessage } = renderSurface(store);

    await user.click(screen.getByTestId("kit-button"));

    expect(store.envelopeCalls).toHaveLength(1);
    expect(store.envelopeCalls[0].action).toBe(EVENT_ACTION);
    expect(store.envelopeCalls[0].source).toBe("button1");
    expect(onClientMessage).toHaveBeenCalledTimes(1);
    expect(onClientMessage.mock.calls[0][0]).toMatchObject({
      action: { name: "submit", surfaceId: "s1" },
    });
  });

  it("FunctionCall 型 Button：不生成信封、不回传（M1 本地处理留 TODO）", async () => {
    const user = userEvent.setup();
    const fnAction: Action = { call: "localFn", args: {} };
    const store = makeMockStore({
      nodes: buildTree(fnAction),
      rootId: "card1",
    });
    const { onClientMessage } = renderSurface(store);

    await user.click(screen.getByTestId("kit-button"));

    expect(store.envelopeCalls).toHaveLength(0);
    expect(onClientMessage).not.toHaveBeenCalled();
  });

  it("未知 component 回退到 Placeholder", () => {
    const store = makeMockStore({
      nodes: {
        rootX: { id: "rootX", component: "Slider", props: {}, children: [] },
      },
      rootId: "rootX",
    });
    renderSurface(store);
    expect(screen.getByTestId("kit-placeholder")).toBeInTheDocument();
    expect(screen.getByTestId("kit-placeholder")).toHaveTextContent(/Slider/);
  });

  it("resolved.placeholder 非空时渲染 Placeholder（即使 component 已知）", () => {
    const store = makeMockStore({
      nodes: {
        t: {
          id: "t",
          component: "Text",
          props: { text: "x", variant: "body" },
          children: [],
          placeholder: "reference missing",
        },
      },
      rootId: "t",
    });
    renderSurface(store);
    expect(screen.getByTestId("kit-placeholder")).toHaveTextContent("reference missing");
  });

  it("root 缺失时渲染空", () => {
    const store = makeMockStore({ nodes: {} });
    const { container } = render(
      <A2UIProvider store={store} kit={makeMockKit()}>
        <Surface surfaceId="s1" />
      </A2UIProvider>,
    );
    expect(container.querySelector("[data-testid]")).toBeNull();
  });

  it("Button 的 label 内嵌子组件时递归渲染 children 作为 label", () => {
    const store = makeMockStore({
      nodes: {
        b: {
          id: "b",
          component: "Button",
          props: { variant: "default" },
          children: [ref("inner")],
          action: EVENT_ACTION,
        },
        inner: {
          id: "inner",
          component: "Text",
          props: { text: "Inner", variant: "caption" },
          children: [],
        },
      },
      rootId: "b",
    });
    renderSurface(store);
    const button = screen.getByTestId("kit-button");
    expect(button).toContainElement(screen.getByTestId("kit-text"));
    expect(screen.getByTestId("kit-text")).toHaveTextContent("Inner");
  });
});
