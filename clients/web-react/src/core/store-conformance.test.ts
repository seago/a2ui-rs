/**
 * 一致性向量 runner（SurfaceStore 契约版）：读取仓库根 `tests/conformance/*.json`，
 * 用 {@link createSurfaceStore} 依次 ingest 每条 envelope，再从 root NodeRef 惰性展开
 * 整棵树，断言最终 Data Model、关键组件解析结果、与规范化树等于 `expect`。
 *
 * 这些 JSON 用例语言无关（不含 TS 专有结构），未来 Rust 侧亦会写 runner 消费同一批。
 */
import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createSurfaceStore } from "@/core/store";
import type { SurfaceStore, NodeRef } from "@/contracts/store";
import type { ServerEnvelope } from "@/contracts/protocol";

type Json =
  | null
  | boolean
  | number
  | string
  | Json[]
  | { [key: string]: Json };

interface NormalizedNode {
  id: string;
  type: string;
  props: Record<string, Json>;
  children: NormalizedNode[];
  placeholder?: true;
}

interface ConformanceCase {
  name: string;
  surfaceId?: string;
  messages: ServerEnvelope[];
  expect: {
    dataModel?: Json;
    resolved?: Record<string, Record<string, Json>>;
    tree?: NormalizedNode;
  };
}

/** 从一个 NodeRef 惰性展开为规范化树（component→type，template 实例 id 加 `#index`）。 */
function normalizeFrom(store: SurfaceStore, surfaceId: string, ref: NodeRef): NormalizedNode {
  const node = store.resolveNode(surfaceId, ref)!;
  // template 实例：id 取 `${componentId}#${最内层帧索引}`；普通节点用组件 id。
  const frames = ref.scope.frames;
  const id =
    frames.length > 0 ? `${node.id}#${frames[frames.length - 1].index}` : node.id;
  const out: NormalizedNode = {
    id,
    type: node.placeholder ? "__placeholder__" : node.component,
    props: node.props as Record<string, Json>,
    children: node.children.map((c) => normalizeFrom(store, surfaceId, c)),
  };
  if (node.placeholder) out.placeholder = true;
  return out;
}

const cwd = (globalThis as { process?: { cwd(): string } }).process!.cwd();
const dir = resolve(cwd, "../../tests/conformance") + "/";
const files = readdirSync(dir)
  .filter((f) => f.endsWith(".json"))
  .sort();

describe("SurfaceStore conformance vectors", () => {
  it("finds vector files", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  for (const file of files) {
    const testCase = JSON.parse(
      readFileSync(`${dir}${file}`, "utf8"),
    ) as ConformanceCase;
    const surfaceId = testCase.surfaceId ?? "s1";

    it(`${file}: ${testCase.name}`, () => {
      const store = createSurfaceStore();
      for (const msg of testCase.messages) store.ingest(msg);

      if (testCase.expect.dataModel !== undefined) {
        expect(store.getDataValue(surfaceId, "/")).toEqual(testCase.expect.dataModel);
      }

      if (testCase.expect.resolved) {
        for (const [id, props] of Object.entries(testCase.expect.resolved)) {
          const node = store.resolveNode(surfaceId, {
            componentId: id,
            scope: { frames: [] },
          });
          expect(node, `component ${id} should resolve`).toBeDefined();
          expect(node!.props).toEqual(props);
        }
      }

      if (testCase.expect.tree) {
        const root = store.getRootRef(surfaceId)!;
        expect(normalizeFrom(store, surfaceId, root)).toEqual(testCase.expect.tree);
      }
    });
  }
});
