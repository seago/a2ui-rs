/**
 * `createSurfaceStore` —— 对 `@/contracts/store` 的 SurfaceStore 契约的实现测试。
 *
 * 严格按契约签名（store.ts）与协议线格式（protocol.ts）驱动，覆盖：
 * createSurface 建树、DynamicValue 求值、bindingPath、ChildList 模板 + @index、
 * updateDataModel upsert/删除、响应性订阅、buildActionEnvelope（含 sendDataModel）。
 */
import { describe, it, expect, vi } from "vitest";
import { createSurfaceStore } from "@/core/store";
import { ROOT_SCOPE } from "@/contracts/store";
import type { ServerEnvelope } from "@/contracts/protocol";
import type { NodeRef, Scope } from "@/contracts/store";

/** 目标 demo 消息（serve_demo 会推的那条）。 */
const demoEnvelope: ServerEnvelope = {
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
        action: { name: "submit", wantResponse: true, actionId: "submit-1" },
      },
      { id: "submit_label", component: "Text", text: "提交" },
    ],
  },
};

describe("createSurfaceStore — lifecycle & snapshot", () => {
  it("ingests createSurface and exposes an active snapshot", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    expect(store.getSurfaceIds()).toEqual(["s1"]);
    const snap = store.getSurface("s1");
    expect(snap?.surfaceId).toBe("s1");
    expect(snap?.catalogId).toBe("basic");
    expect(snap?.lifecycle).toBe("active");
    expect(snap?.root).toEqual<NodeRef>({
      componentId: "root_card",
      scope: ROOT_SCOPE,
    });
  });

  it("getRootRef returns the root node ref", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    expect(store.getRootRef("s1")).toEqual<NodeRef>({
      componentId: "root_card",
      scope: ROOT_SCOPE,
    });
  });

  it("deleteSurface marks lifecycle deleted", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    store.ingest({ version: "v1.0", deleteSurface: { surfaceId: "s1" } });
    expect(store.getSurface("s1")?.lifecycle).toBe("deleted");
  });

  it("is robust to malformed / unknown envelopes (no throw)", () => {
    const store = createSurfaceStore();
    expect(() => store.ingest({} as ServerEnvelope)).not.toThrow();
    expect(() => store.ingest({ version: "v1.0" } as ServerEnvelope)).not.toThrow();
    expect(() =>
      store.ingest({ version: "v0.9", createSurface: { surfaceId: "x", catalogId: "b" } } as unknown as ServerEnvelope),
    ).not.toThrow();
    expect(store.getSurfaceIds()).toEqual([]);
  });
});

describe("createSurfaceStore — resolveNode", () => {
  const store = createSurfaceStore();
  store.ingest(demoEnvelope);
  const ref = (id: string, scope: Scope = ROOT_SCOPE): NodeRef => ({
    componentId: id,
    scope,
  });

  it("resolves Card root with single child ref", () => {
    const node = store.resolveNode("s1", ref("root_card"))!;
    expect(node.component).toBe("Card");
    expect(node.children).toEqual([ref("form_col")]);
  });

  it("resolves Column with static children refs", () => {
    const node = store.resolveNode("s1", ref("form_col"))!;
    expect(node.children).toEqual([
      ref("title_text"),
      ref("name_field"),
      ref("submit_btn"),
    ]);
  });

  it("resolves literal Text prop", () => {
    const node = store.resolveNode("s1", ref("title_text"))!;
    expect(node.component).toBe("Text");
    expect(node.props.text).toBe("请输入你的名字");
  });

  it("resolves TextField value via path binding and reports bindingPath", () => {
    const node = store.resolveNode("s1", ref("name_field"))!;
    expect(node.props.value).toBe("");
    expect(node.props.label).toBe("姓名");
    expect(node.props.variant).toBe("shortText");
    expect(node.bindingPath).toBe("/form/name");
  });

  it("resolves Button action and single child", () => {
    const node = store.resolveNode("s1", ref("submit_btn"))!;
    expect(node.props.variant).toBe("primary");
    expect(node.action).toEqual({
      name: "submit",
      wantResponse: true,
      actionId: "submit-1",
    });
    expect(node.children).toEqual([ref("submit_label")]);
  });

  it("returns a placeholder node for missing references", () => {
    const s2 = createSurfaceStore();
    s2.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s2",
        catalogId: "basic",
        components: [{ id: "root", component: "Card", child: "ghost" }],
      },
    });
    const root = s2.resolveNode("s2", ref("root"))!;
    expect(root.children).toEqual([ref("ghost")]);
    const ghost = s2.resolveNode("s2", ref("ghost"));
    expect(ghost?.placeholder).toBeTruthy();
  });

  it("buffers when root has not arrived yet", () => {
    const s3 = createSurfaceStore();
    s3.ingest({
      version: "v1.0",
      createSurface: { surfaceId: "s3", catalogId: "basic", components: [] },
    });
    expect(s3.getRootRef("s3")).toBeUndefined();
    expect(s3.getSurface("s3")?.root).toBeUndefined();
  });
});

describe("createSurfaceStore — Data Model & reactivity", () => {
  it("reads and writes data values (view -> model)", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    expect(store.getDataValue("s1", "/form/name")).toBe("");
    store.setDataValue("s1", "/form/name", "张三");
    expect(store.getDataValue("s1", "/form/name")).toBe("张三");
    const node = store.resolveNode("s1", { componentId: "name_field", scope: ROOT_SCOPE })!;
    expect(node.props.value).toBe("张三");
  });

  it("notifies subscribers on setDataValue and updateDataModel", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    const listener = vi.fn();
    const off = store.subscribe(listener);
    store.setDataValue("s1", "/form/name", "李四");
    expect(listener).toHaveBeenCalledTimes(1);
    store.ingest({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/form/name", value: "王五" },
    });
    expect(listener).toHaveBeenCalledTimes(2);
    off();
    store.setDataValue("s1", "/form/name", "赵六");
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("updateDataModel omitting value deletes the key (vs null which sets)", () => {
    const store = createSurfaceStore();
    store.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        dataModel: { a: 1, b: 2 },
        components: [{ id: "root", component: "Text", text: "x" }],
      },
    });
    store.ingest({ version: "v1.0", updateDataModel: { surfaceId: "s1", path: "/a" } });
    store.ingest({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/b", value: null },
    });
    expect(store.getDataValue("s1", "/")).toEqual({ b: null });
  });
});

describe("createSurfaceStore — ChildList template + @index + collection scope", () => {
  const store = createSurfaceStore();
  store.ingest({
    version: "v1.0",
    createSurface: {
      surfaceId: "s1",
      catalogId: "basic",
      dataModel: { items: [{ label: "苹果" }, { label: "香蕉" }, { label: "橙子" }] },
      components: [
        { id: "root", component: "Column", children: { template: "row_tpl", path: "/items" } },
        {
          id: "row_tpl",
          component: "Text",
          text: {
            call: "formatString",
            args: {
              template: "{i}. {label}",
              bindings: { i: { call: "@index" }, label: { path: "label" } },
            },
          },
        },
      ],
    },
  });

  it("expands template into one child NodeRef per array item with collection scope", () => {
    const root = store.resolveNode("s1", { componentId: "root", scope: ROOT_SCOPE })!;
    expect(root.children).toHaveLength(3);
    expect(root.children.map((r) => r.componentId)).toEqual([
      "row_tpl",
      "row_tpl",
      "row_tpl",
    ]);
    expect(root.children.map((r) => r.scope.frames)).toEqual([
      [{ basePath: "/items", index: 0 }],
      [{ basePath: "/items", index: 1 }],
      [{ basePath: "/items", index: 2 }],
    ]);
  });

  it("resolves @index and relative path under the collection scope", () => {
    const root = store.resolveNode("s1", { componentId: "root", scope: ROOT_SCOPE })!;
    const texts = root.children.map(
      (childRef) => store.resolveNode("s1", childRef)!.props.text,
    );
    expect(texts).toEqual(["0. 苹果", "1. 香蕉", "2. 橙子"]);
  });
});

describe("createSurfaceStore — buildActionEnvelope", () => {
  it("builds an action envelope and attaches dataModel metadata when sendDataModel", () => {
    const store = createSurfaceStore();
    store.ingest(demoEnvelope);
    store.setDataValue("s1", "/form/name", "张三");
    const node = store.resolveNode("s1", { componentId: "submit_btn", scope: ROOT_SCOPE })!;
    const env = store.buildActionEnvelope("s1", node.action!, "submit_btn");
    expect(env.version).toBe("v1.0");
    expect(env.action).toMatchObject({
      name: "submit",
      surfaceId: "s1",
      sourceComponentId: "submit_btn",
      wantResponse: true,
      actionId: "submit-1",
    });
    expect(env.metadata).toEqual({ surfaceId: "s1", dataModel: { form: { name: "张三" } } });
  });

  it("omits metadata when sendDataModel is false", () => {
    const store = createSurfaceStore();
    store.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        components: [
          {
            id: "root",
            component: "Button",
            action: { name: "go" },
            child: "l",
          },
          { id: "l", component: "Text", text: "go" },
        ],
      },
    });
    const env = store.buildActionEnvelope("s1", { name: "go" }, "root");
    expect(env.action?.name).toBe("go");
    expect(env.metadata).toBeUndefined();
  });

  it("resolves context relative paths / @index against the provided scope", () => {
    const store = createSurfaceStore();
    store.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        sendDataModel: false,
        dataModel: { items: [{ id: "a" }, { id: "b" }] },
        components: [
          { id: "root", component: "Column", children: { template: "row", path: "/items" } },
          { id: "row", component: "Text", text: "x" },
        ],
      },
    });
    const scope: Scope = { frames: [{ basePath: "/items", index: 1 }] };
    const action = {
      name: "pick",
      context: { itemId: { path: "id" }, i: { call: "@index" } },
    };
    const env = store.buildActionEnvelope("s1", action, "row", scope);
    expect(env.action?.context).toEqual({ itemId: "b", i: 1 });
  });
});
