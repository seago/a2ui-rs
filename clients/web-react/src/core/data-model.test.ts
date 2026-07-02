import { describe, it, expect } from "vitest";
import { DataModel } from "@/core/data-model";
import {
  resolvePointer,
  applyPointer,
  validatePointer,
  escapeToken,
  unescapeToken,
  parseArrayIndex,
  PointerError,
} from "@/core/json-pointer";

describe("json-pointer helpers", () => {
  it("escapes and unescapes tokens (RFC6901)", () => {
    expect(escapeToken("a/b")).toBe("a~1b");
    expect(escapeToken("a~b")).toBe("a~0b");
    expect(unescapeToken("a~1b")).toBe("a/b");
    expect(unescapeToken("a~0b")).toBe("a~b");
    // 顺序：~01 应还原为 ~1 而非 /
    expect(unescapeToken("~01")).toBe("~1");
  });

  it("parses array indices, rejecting leading zeros", () => {
    expect(parseArrayIndex("0")).toBe(0);
    expect(parseArrayIndex("12")).toBe(12);
    expect(parseArrayIndex("01")).toBeNull();
    expect(parseArrayIndex("")).toBeNull();
    expect(parseArrayIndex("x")).toBeNull();
  });

  it("resolvePointer reads nested values and array indices", () => {
    const root = { items: [{ id: 1 }, { id: 2 }] };
    expect(resolvePointer(root, "/items/0/id")).toBe(1);
    expect(resolvePointer(root, "/items/1/id")).toBe(2);
    expect(resolvePointer(root, "/missing")).toBeUndefined();
    expect(resolvePointer(root, "")).toBe(root);
    expect(resolvePointer(root, "/")).toBe(root);
  });

  it("resolvePointer honors escaped keys", () => {
    expect(resolvePointer({ "a/b": "x" }, "/a~1b")).toBe("x");
    expect(resolvePointer({ "a~b": "y" }, "/a~0b")).toBe("y");
  });

  it("validatePointer rejects malicious paths", () => {
    expect(() => validatePointer("/a\0/b")).toThrow(PointerError);
    expect(() => validatePointer("/a//b")).toThrow(PointerError);
    expect(() => validatePointer("/a/../b")).toThrow(PointerError);
    expect(() => validatePointer("/ok/path")).not.toThrow();
  });

  it("applyPointer replaces root", () => {
    const next = applyPointer({ old: true }, "/", true, { new: true });
    expect(next).toEqual({ new: true });
  });
});

describe("DataModel", () => {
  it("creates and reads values", () => {
    const dm = new DataModel({ name: "Alice" });
    expect(dm.resolvePointer("/name")).toBe("Alice");
    expect(dm.value).toEqual({ name: "Alice" });
  });

  it("upserts (create) at a new path", () => {
    const dm = new DataModel({});
    dm.applyPointer("/name", "Alice");
    expect(dm.resolvePointer("/name")).toBe("Alice");
  });

  it("upserts (update) an existing path", () => {
    const dm = new DataModel({ name: "Alice" });
    dm.applyPointer("/name", "Bob");
    expect(dm.resolvePointer("/name")).toBe("Bob");
  });

  it("creates intermediate nodes", () => {
    const dm = new DataModel({});
    dm.applyPointer("/user/name", "Alice");
    expect(dm.resolvePointer("/user/name")).toBe("Alice");
  });

  it("creates arrays when next segment is an index", () => {
    const dm = new DataModel({ arr: [{ id: 1 }, { id: 2 }] });
    dm.applyPointer("/arr/0/name", "first");
    expect(dm.resolvePointer("/arr/0/name")).toBe("first");
    expect(dm.resolvePointer("/arr/0/id")).toBe(1);
  });

  it("deletes a key when value omitted", () => {
    const dm = new DataModel({ name: "Alice", keep: 1 });
    dm.applyPointer("/name");
    expect(dm.resolvePointer("/name")).toBeUndefined();
    expect(dm.resolvePointer("/keep")).toBe(1);
  });

  it("distinguishes explicit null (set) from omitted (delete)", () => {
    const dm = new DataModel({ a: 1 });
    dm.applyPointer("/a", null);
    expect(dm.resolvePointer("/a")).toBeNull();
    const dm2 = new DataModel({ a: 1 });
    dm2.applyPointer("/a");
    expect(dm2.resolvePointer("/a")).toBeUndefined();
  });

  it("splices array elements on delete", () => {
    const dm = new DataModel({ arr: ["a", "b", "c"] });
    dm.applyPointer("/arr/1");
    expect(dm.resolvePointer("/arr/0")).toBe("a");
    expect(dm.resolvePointer("/arr/1")).toBe("c");
  });

  it("replaces the whole model on root pointer", () => {
    const dm = new DataModel({ old: true });
    dm.applyPointer("/", { fresh: true });
    expect(dm.value).toEqual({ fresh: true });
  });

  it("clears the model on root delete", () => {
    const dm = new DataModel({ a: 1 });
    dm.applyPointer("/");
    expect(dm.value).toEqual({});
  });

  it("throws on malicious write paths", () => {
    const dm = new DataModel({ a: { b: 1 } });
    expect(() => dm.applyPointer("/a//b", 2)).toThrow(PointerError);
    expect(() => dm.applyPointer("/a/../b", 2)).toThrow(PointerError);
  });

  it("reports change descriptors", () => {
    const dm = new DataModel({});
    expect(dm.applyPointer("/x", 1)).toEqual({ path: "/x", deleted: false });
    expect(dm.applyPointer("/x")).toEqual({ path: "/x", deleted: true });
    expect(dm.applyPointer("/", { y: 2 })).toEqual({ path: "/", deleted: false });
  });
});
