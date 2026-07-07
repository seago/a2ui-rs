import { describe, it, expect, vi } from "vitest";
import { A2uiEngine } from "@/core/engine";
import type { Json } from "@/core/types";

/**
 * 示例组件树（Card→Column→[Text, TextField, Button→Text]）的**扁平 wire 形态**。
 * 注意：喂入 ingest 的组件是扁平邻接表（特有属性与 id/component 平铺），由核心层解析。
 */
function demoComponents(): Record<string, Json>[] {
  return [
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
      action: { event: { name: "submit", wantResponse: true, actionId: "submit-1" } },
    },
    { id: "submit_label", component: "Text", text: "提交" },
  ];
}

function demoEnvelope(sendDataModel = true) {
  return {
    version: "v1.0",
    createSurface: {
      surfaceId: "s1",
      catalogId: "basic",
      sendDataModel,
      components: demoComponents(),
      dataModel: { form: { name: "" } },
    },
  };
}

describe("createSurface + forest", () => {
  it("ingests the demo surface and builds the tree from root_card", () => {
    const engine = new A2uiEngine();
    const res = engine.ingest(demoEnvelope());
    expect(res.ok).toBe(true);
    const s = engine.getSurface("s1")!;
    expect(s.state).toBe("active");
    expect(s.getRootId()).toBe("root_card");
    expect(s.getDataModel()).toEqual({ form: { name: "" } });

    const tree = s.getRenderTree()!;
    expect(tree.type).toBe("Card");
    expect(tree.children[0].type).toBe("Column");
    const col = tree.children[0];
    expect(col.children.map((c) => c.type)).toEqual([
      "Text",
      "TextField",
      "Button",
    ]);
    // Button → Text("提交")
    const btn = col.children[2];
    expect(btn.children[0].props.text).toBe("提交");
  });

  it("resolves component props at root scope", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope());
    const s = engine.getSurface("s1")!;
    expect(s.resolveComponent("title_text")!.props).toEqual({
      text: "请输入你的名字",
    });
    expect(s.resolveComponent("name_field")!.props).toEqual({
      value: "",
      label: "姓名",
      variant: "shortText",
    });
    expect(s.resolveComponent("submit_btn")!.props).toEqual({
      variant: "primary",
    });
  });
});

describe("progressive rendering", () => {
  it("buffers until root and leaves placeholders for missing refs", () => {
    const engine = new A2uiEngine();
    engine.ingest({
      version: "v1.0",
      createSurface: { surfaceId: "s1", catalogId: "basic", dataModel: {} },
    });
    const s = engine.getSurface("s1")!;
    expect(s.getRenderTree()).toBeUndefined(); // no root yet

    engine.ingest({
      version: "v1.0",
      updateComponents: {
        surfaceId: "s1",
        components: [
          { id: "root", component: "Card", child: "missing_child" },
        ],
      },
    });
    const tree = s.getRenderTree()!;
    expect(tree.type).toBe("Card");
    expect(tree.children[0].placeholder).toBe(true);

    // 补齐缺失引用后占位被替换
    engine.ingest({
      version: "v1.0",
      updateComponents: {
        surfaceId: "s1",
        components: [{ id: "missing_child", component: "Text", text: "ok" }],
      },
    });
    expect(s.getRenderTree()!.children[0].props.text).toBe("ok");
  });
});

describe("updateDataModel + reactivity", () => {
  it("upserts and notifies dependents", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope());
    const s = engine.getSurface("s1")!;
    // 先解析登记依赖
    s.resolveComponent("name_field");

    const listener = vi.fn();
    s.subscribe(listener);
    engine.ingest({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/form/name", value: "张三" },
    });
    expect(s.getDataValue("/form/name")).toBe("张三");
    expect(listener).toHaveBeenCalledTimes(1);
    const note = listener.mock.calls[0][0];
    expect(note.path).toBe("/form/name");
    expect(note.affected.has("name_field")).toBe(true);
    // 重新解析反映新值
    expect(s.resolveComponent("name_field")!.props.value).toBe("张三");
  });

  it("deletes a path when value omitted", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope());
    const s = engine.getSurface("s1")!;
    engine.ingest({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/form/name" },
    });
    expect(s.getDataValue("/form/name")).toBeUndefined();
  });

  it("component-level subscription fires only for dependents", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope());
    const s = engine.getSurface("s1")!;
    s.resolveComponent("name_field");
    s.resolveComponent("title_text");
    const nameListener = vi.fn();
    const titleListener = vi.fn();
    s.subscribeComponent("name_field", nameListener);
    s.subscribeComponent("title_text", titleListener);
    s.setDataValue("/form/name", "李四");
    expect(nameListener).toHaveBeenCalledTimes(1);
    expect(titleListener).not.toHaveBeenCalled();
  });
});

describe("ChildList template + @index", () => {
  it("expands template rows with collection scope and @index", () => {
    const engine = new A2uiEngine();
    engine.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        dataModel: { items: [{ label: "a" }, { label: "b" }] },
        components: [
          {
            id: "root",
            component: "Column",
            children: { template: "row_tpl", path: "/items" },
          },
          {
            id: "row_tpl",
            component: "Text",
            text: {
              call: "formatString",
              args: {
                template: "{i}:{label}",
                bindings: { i: { call: "@index" }, label: { path: "label" } },
              },
            },
          },
        ],
      },
    });
    const s = engine.getSurface("s1")!;
    const tree = s.getRenderTree()!;
    expect(tree.children.map((c) => c.id)).toEqual(["row_tpl#0", "row_tpl#1"]);
    expect(tree.children.map((c) => c.props.text)).toEqual(["0:a", "1:b"]);
  });
});

describe("buildActionMessage + actionResponse writeback", () => {
  const ISO_SECONDS = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;

  it("builds a ClientEnvelope and attaches dataModel when sendDataModel", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope(true));
    engine.getSurface("s1")!.setDataValue("/form/name", "王五");
    const dispatch = engine.buildActionMessage("s1", "submit_btn")!;
    expect(dispatch.envelope).toMatchObject({
      version: "v1.0",
      action: {
        name: "submit",
        surfaceId: "s1",
        sourceComponentId: "submit_btn",
        wantResponse: true,
        actionId: "submit-1",
      },
    });
    // 规范：timestamp 必填（秒精度 ISO 8601）
    if ("action" in dispatch.envelope) {
      expect(dispatch.envelope.action.timestamp).toMatch(ISO_SECONDS);
    }
    expect(dispatch.dataModel).toEqual({ form: { name: "王五" } });
  });

  it("omits dataModel when sendDataModel is false", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope(false));
    const dispatch = engine.buildActionMessage("s1", "submit_btn")!;
    expect(dispatch.dataModel).toBeUndefined();
  });

  it("writes actionResponse value back to responsePath (kept off the wire)", () => {
    const engine = new A2uiEngine();
    engine.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        dataModel: {},
        components: [
          {
            id: "root",
            component: "Button",
            child: "lbl",
            action: {
              event: {
                name: "go",
                wantResponse: true,
                actionId: "a1",
                responsePath: "/result",
              },
            },
          },
          { id: "lbl", component: "Text", text: "go" },
        ],
      },
    });
    const dispatch = engine.buildActionMessage("s1", "root")!; // 登记 pending
    // D4：responsePath 不上线路
    if ("action" in dispatch.envelope) {
      expect("responsePath" in dispatch.envelope.action).toBe(false);
    }
    // 规范 wire：actionId 在信封层，payload 只含 value/error
    engine.ingest({
      version: "v1.0",
      actionId: "a1",
      actionResponse: { value: { ok: true } },
    });
    expect(engine.getSurface("s1")!.getDataValue("/result")).toEqual({
      ok: true,
    });
  });

  it("auto-generates an actionId when wantResponse is declared without one", () => {
    const engine = new A2uiEngine();
    engine.ingest({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        dataModel: {},
        components: [
          {
            id: "root",
            component: "Button",
            child: "lbl",
            action: {
              event: { name: "go", wantResponse: true, responsePath: "/result" },
            },
          },
          { id: "lbl", component: "Text", text: "go" },
        ],
      },
    });
    const dispatch = engine.buildActionMessage("s1", "root")!;
    if (!("action" in dispatch.envelope)) throw new Error("expected action");
    const actionId = dispatch.envelope.action.actionId;
    expect(actionId).toBeTruthy();
    engine.ingest({
      version: "v1.0",
      actionId,
      actionResponse: { value: 7 },
    });
    expect(engine.getSurface("s1")!.getDataValue("/result")).toBe(7);
  });
});

describe("callFunction enforcement", () => {
  it("rejects clientOnly builtin invoked from remote", () => {
    const engine = new A2uiEngine();
    const res = engine.ingest({
      version: "v1.0",
      callFunction: {
        functionCallId: "fc1",
        wantResponse: true,
        call: "required",
        args: { value: "x" },
      },
    });
    expect(res.ok).toBe(false);
    expect(res.reply).toBeDefined();
    if (res.reply && "error" in res.reply) {
      expect(res.reply.error.code).toBe("INVALID_FUNCTION_CALL");
    }
  });

  it("executes clientOrRemote function and returns functionResponse", () => {
    const engine = new A2uiEngine();
    engine.functions.register("add", "clientOrRemote", (args) => {
      const a = (args.a as number) ?? 0;
      const b = (args.b as number) ?? 0;
      return a + b;
    });
    const res = engine.ingest({
      version: "v1.0",
      callFunction: {
        functionCallId: "fc2",
        wantResponse: true,
        call: "add",
        args: { a: 2, b: 3 },
      },
    });
    expect(res.ok).toBe(true);
    if (res.reply && "functionResponse" in res.reply) {
      expect(res.reply.functionResponse.value as Json).toBe(5);
    } else {
      throw new Error("expected functionResponse");
    }
  });
});

describe("lifecycle", () => {
  it("marks surface deleted and ignores further updates", () => {
    const engine = new A2uiEngine();
    engine.ingest(demoEnvelope());
    engine.ingest({ version: "v1.0", deleteSurface: { surfaceId: "s1" } });
    const s = engine.getSurface("s1")!;
    expect(s.state).toBe("deleted");
    engine.ingest({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/form/name", value: "x" },
    });
    expect(s.getDataValue("/form/name")).toBe("");
  });
});
