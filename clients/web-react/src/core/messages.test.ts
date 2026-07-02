import { describe, it, expect } from "vitest";
import { parseServerEnvelope, parseComponent } from "@/core/messages";

describe("parseComponent", () => {
  it("splits fixed fields from properties", () => {
    const c = parseComponent({
      id: "t",
      component: "Text",
      text: "hi",
      weight: 2,
      accessibility: { label: "L" },
    });
    expect(c).not.toBeNull();
    expect(c!.id).toBe("t");
    expect(c!.component).toBe("Text");
    expect(c!.weight).toBe(2);
    expect(c!.accessibility).toEqual({ label: "L", description: undefined });
    expect(c!.properties).toEqual({ text: "hi" });
  });

  it("returns null on missing id/component", () => {
    expect(parseComponent({ component: "Text" })).toBeNull();
    expect(parseComponent({ id: "x" })).toBeNull();
    expect(parseComponent(42)).toBeNull();
  });
});

describe("parseServerEnvelope", () => {
  it("parses createSurface from string", () => {
    const r = parseServerEnvelope(
      '{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}',
    );
    expect(r.ok).toBe(true);
    if (r.ok && r.message.kind === "createSurface") {
      expect(r.message.message.surfaceId).toBe("s1");
      expect(r.message.message.catalogId).toBe("basic");
      expect(r.message.message.sendDataModel).toBe(false);
    } else {
      throw new Error("wrong variant");
    }
  });

  it("parses createSurface components + dataModel", () => {
    const r = parseServerEnvelope({
      version: "v1.0",
      createSurface: {
        surfaceId: "s1",
        catalogId: "basic",
        sendDataModel: true,
        components: [{ id: "root", component: "Text", text: "hi" }],
        dataModel: { form: { name: "" } },
      },
    });
    expect(r.ok).toBe(true);
    if (r.ok && r.message.kind === "createSurface") {
      expect(r.message.message.sendDataModel).toBe(true);
      expect(r.message.message.components).toHaveLength(1);
      expect(r.message.message.dataModel).toEqual({ form: { name: "" } });
    }
  });

  it("distinguishes updateDataModel delete vs set-null", () => {
    const del = parseServerEnvelope({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/a" },
    });
    const setNull = parseServerEnvelope({
      version: "v1.0",
      updateDataModel: { surfaceId: "s1", path: "/a", value: null },
    });
    if (del.ok && del.message.kind === "updateDataModel") {
      expect(del.message.message.hasValue).toBe(false);
    }
    if (setNull.ok && setNull.message.kind === "updateDataModel") {
      expect(setNull.message.message.hasValue).toBe(true);
      expect(setNull.message.message.value).toBeNull();
    }
  });

  it("parses actionResponse success and error", () => {
    const ok = parseServerEnvelope({
      version: "v1.0",
      actionResponse: { actionId: "a1", value: "done" },
    });
    const err = parseServerEnvelope({
      version: "v1.0",
      actionResponse: {
        actionId: "a1",
        error: { code: "E", message: "bad" },
      },
    });
    if (ok.ok && ok.message.kind === "actionResponse") {
      expect(ok.message.message.value).toBe("done");
      expect(ok.message.message.error).toBeUndefined();
    }
    if (err.ok && err.message.kind === "actionResponse") {
      expect(err.message.message.error).toEqual({ code: "E", message: "bad" });
    }
  });

  it("parses callFunction", () => {
    const r = parseServerEnvelope({
      version: "v1.0",
      callFunction: {
        functionCallId: "fc1",
        wantResponse: true,
        call: "required",
        args: { value: "x" },
      },
    });
    if (r.ok && r.message.kind === "callFunction") {
      expect(r.message.message.functionCallId).toBe("fc1");
      expect(r.message.message.call).toBe("required");
      expect(r.message.message.wantResponse).toBe(true);
    }
  });

  it("rejects unknown version", () => {
    const r = parseServerEnvelope({ version: "v9.9", createSurface: {} });
    expect(r.ok).toBe(false);
  });

  it("rejects unknown / missing message key", () => {
    expect(parseServerEnvelope({ version: "v1.0", unknownMessage: {} }).ok).toBe(
      false,
    );
    expect(parseServerEnvelope({ version: "v1.0" }).ok).toBe(false);
  });

  it("rejects multiple message keys", () => {
    const r = parseServerEnvelope({
      version: "v1.0",
      createSurface: { surfaceId: "s", catalogId: "b" },
      deleteSurface: { surfaceId: "s" },
    });
    expect(r.ok).toBe(false);
  });

  it("rejects invalid JSON string", () => {
    expect(parseServerEnvelope("{not json").ok).toBe(false);
  });
});
