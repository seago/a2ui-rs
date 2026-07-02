import { describe, it, expect } from "vitest";
import { DataModel } from "@/core/data-model";
import { PathResolver } from "@/core/path-resolver";
import { formatString, htmlEscape, valueToString } from "@/core/format-string";
import { FunctionDispatcher, FunctionError } from "@/core/functions";
import { resolveValue, resolveComponentProps } from "@/core/resolve";
import type { Component, Json } from "@/core/types";

function ctx(dm: DataModel) {
  return {
    resolver: new PathResolver(dm),
    dispatcher: new FunctionDispatcher(),
    deps: new Set<string>(),
  };
}

describe("PathResolver", () => {
  it("resolves absolute paths", () => {
    const r = new PathResolver(new DataModel({ user: { name: "Alice" } }));
    expect(r.resolve("/user/name")).toBe("Alice");
    expect(r.resolve("/missing")).toBeUndefined();
  });

  it("resolves relative paths in root scope", () => {
    const r = new PathResolver(new DataModel({ name: "Alice" }));
    expect(r.resolve("name")).toBe("Alice");
    expect(r.makeAbsolute("name")).toBe("/name");
  });

  it("resolves relative paths and @index in collection scope", () => {
    const r = new PathResolver(
      new DataModel({ items: [{ name: "a" }, { name: "b" }] }),
    );
    r.enterCollection("/items", 1);
    expect(r.resolve("name")).toBe("b");
    expect(r.makeAbsolute("name")).toBe("/items/1/name");
    expect(r.resolve("@index")).toBe(1);
    r.exitCollection();
    expect(r.resolve("@index")).toBeUndefined();
  });

  it("withCollection restores scope on throw", () => {
    const r = new PathResolver(new DataModel({ items: [{}, {}] }));
    expect(() =>
      r.withCollection("/items", 0, () => {
        throw new Error("boom");
      }),
    ).toThrow("boom");
    expect(r.currentIndex()).toBeUndefined();
  });
});

describe("formatString", () => {
  it("interpolates {key} with type conversion", () => {
    expect(formatString("{a}-{b}", { a: "x", b: 3 })).toBe("x-3");
    expect(formatString("{flag}", { flag: true })).toBe("true");
  });

  it("handles unknown keys and escaped braces", () => {
    expect(formatString("{missing}!", {})).toBe("!");
    expect(formatString("{{literal}}", {})).toBe("{literal}");
  });

  it("valueToString / htmlEscape behave as documented", () => {
    expect(valueToString(null)).toBe("");
    expect(valueToString({ a: 1 } as Json)).toBe('{"a":1}');
    expect(htmlEscape('<b>"&\'')).toBe("&lt;b&gt;&quot;&amp;&#39;");
  });
});

describe("FunctionDispatcher", () => {
  it("enforces callableFrom for builtins", () => {
    const d = new FunctionDispatcher();
    expect(d.dispatch("required", { value: "x" }, "client")).toBe(true);
    expect(d.dispatch("required", { value: "" }, "client")).toBe(false);
    // clientOnly rejected from remote
    expect(() => d.dispatch("required", { value: "x" }, "remote")).toThrow(
      FunctionError,
    );
    expect(d.canCall("required", "remote")).toBe(false);
    expect(d.canCall("required", "client")).toBe(true);
  });

  it("rejects unregistered functions", () => {
    const d = new FunctionDispatcher();
    expect(() => d.dispatch("nope", {}, "client")).toThrow(FunctionError);
    expect(d.has("nope")).toBe(false);
  });

  it("formatString builtin is callable from both sides", () => {
    const d = new FunctionDispatcher();
    const out = d.dispatch(
      "formatString",
      { template: "Hi {n}", bindings: { n: "Bob" } },
      "remote",
    );
    expect(out).toBe("Hi Bob");
  });
});

describe("resolveValue / resolveComponentProps", () => {
  it("resolves path bindings and records deps", () => {
    const dm = new DataModel({ user: { name: "Alice" } });
    const c = ctx(dm);
    expect(resolveValue({ path: "/user/name" }, c)).toBe("Alice");
    expect(c.deps.has("/user/name")).toBe(true);
  });

  it("resolves @index and relative deps in collection scope", () => {
    const dm = new DataModel({ items: [{ name: "a" }, { name: "b" }] });
    const c = ctx(dm);
    c.resolver.enterCollection("/items", 1);
    expect(resolveValue({ call: "@index" }, c)).toBe(1);
    expect(resolveValue({ path: "name" }, c)).toBe("b");
    expect(c.deps.has("/items/1/name")).toBe(true);
  });

  it("resolves formatString call with bindings", () => {
    const dm = new DataModel({ user: { name: "Alice" } });
    const c = ctx(dm);
    const out = resolveValue(
      {
        call: "formatString",
        args: {
          template: "Hello, {name}!",
          bindings: { name: { path: "/user/name" } },
        },
      },
      c,
    );
    expect(out).toBe("Hello, Alice!");
    expect(c.deps.has("/user/name")).toBe(true);
  });

  it("degrades unknown function to undefined", () => {
    const dm = new DataModel({});
    const c = ctx(dm);
    expect(resolveValue({ call: "unknownFn", args: {} }, c)).toBeUndefined();
  });

  it("resolves component props, skipping structural keys", () => {
    const dm = new DataModel({ form: { name: "张三" } });
    const comp: Component = {
      id: "name_field",
      component: "TextField",
      properties: {
        value: { path: "/form/name" },
        label: "姓名",
        variant: "shortText",
        // structural — must be ignored by prop resolution
        action: { name: "noop" },
      },
    };
    const { props, deps } = resolveComponentProps(
      comp,
      new PathResolver(dm),
      new FunctionDispatcher(),
    );
    expect(props).toEqual({ value: "张三", label: "姓名", variant: "shortText" });
    expect(deps.has("/form/name")).toBe(true);
  });
});
