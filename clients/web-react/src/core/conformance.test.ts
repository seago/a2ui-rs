/**
 * 一致性向量 runner：读取仓库根 `tests/conformance/*.json`，把 messages 依次 ingest
 * 进核心引擎，断言最终 Data Model 与关键组件解析结果等于 expect。
 *
 * 这些 JSON 用例语言无关，未来 Rust 侧亦会写 runner 消费同一批。
 */
import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { A2uiEngine } from "@/core/engine";
import type { ResolvedNode } from "@/core/surface";
import type { Json } from "@/core/types";

interface ConformanceCase {
  name: string;
  surfaceId?: string;
  messages: unknown[];
  expect: {
    dataModel?: Json;
    resolved?: Record<string, Json>;
    tree?: NormalizedNode;
  };
}

interface NormalizedNode {
  id: string;
  type: string;
  props: Record<string, Json>;
  children: NormalizedNode[];
  placeholder?: true;
}

function normalize(node: ResolvedNode): NormalizedNode {
  const out: NormalizedNode = {
    id: node.id,
    type: node.type,
    props: node.props,
    children: node.children.map(normalize),
  };
  if (node.placeholder) out.placeholder = true;
  return out;
}

// Vitest 运行时 cwd 为 clients/web-react；一致性向量在仓库根 tests/conformance。
const cwd = (globalThis as { process?: { cwd(): string } }).process!.cwd();
const dir = resolve(cwd, "../../tests/conformance") + "/";
const files = readdirSync(dir)
  .filter((f) => f.endsWith(".json"))
  .sort();

describe("conformance vectors", () => {
  it("finds vector files", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  for (const file of files) {
    const testCase = JSON.parse(
      readFileSync(`${dir}${file}`, "utf8"),
    ) as ConformanceCase;

    it(`${file} — ${testCase.name}`, () => {
      const engine = new A2uiEngine();
      let firstSurfaceId: string | undefined;
      for (const env of testCase.messages) {
        const res = engine.ingest(env);
        expect(res.ok, `ingest failed: ${res.error}`).toBe(true);
        const cs = (env as Record<string, { surfaceId?: string }>).createSurface;
        if (cs && firstSurfaceId === undefined) firstSurfaceId = cs.surfaceId;
      }

      const sid = testCase.surfaceId ?? firstSurfaceId!;
      const surface = engine.getSurface(sid);
      expect(surface, `surface not found: ${sid}`).toBeDefined();

      const exp = testCase.expect;
      if (exp.dataModel !== undefined) {
        expect(surface!.getDataModel()).toEqual(exp.dataModel);
      }
      if (exp.resolved) {
        for (const [id, props] of Object.entries(exp.resolved)) {
          const resolved = surface!.resolveComponent(id);
          expect(resolved, `component not resolved: ${id}`).toBeDefined();
          expect(resolved!.props).toEqual(props);
        }
      }
      if (exp.tree) {
        const tree = surface!.getRenderTree();
        expect(tree, "render tree missing").toBeDefined();
        expect(normalize(tree!)).toEqual(exp.tree);
      }
    });
  }
});
