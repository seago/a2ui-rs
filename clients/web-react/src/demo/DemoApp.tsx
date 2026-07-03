import { useMemo, useState } from "react";

import type { ComponentKit } from "@/contracts";
import { createSurfaceStore } from "@/core";
import { shadcnKit } from "@/kits/shadcn";
import { htmlKit } from "@/kits/html";
import { A2UIProvider, Surface } from "@/react";

import { demoEnvelope } from "./surface";

type KitName = "shadcn" | "html";

const KITS: Record<KitName, ComponentKit> = {
  shadcn: shadcnKit,
  html: htmlKit,
};

const KIT_LABEL: Record<KitName, string> = {
  shadcn: "shadcn kit",
  html: "纯 HTML kit",
};

/**
 * B2 示例页：演示「协议核心 + 可插拔 ComponentKit」。
 *
 * 同一条 A2UI 协议消息喂进同一个 store，用顶部的开关实时切换 ComponentKit：
 * 整套组件库随之切换，而表单内容（Data Model / 输入值）保持不变
 * —— 状态活在协议核心，与 kit 无关。
 */
export function DemoApp() {
  // store 只创建并 ingest 一次；切换 kit 不会重建，故状态保留。
  const store = useMemo(() => {
    const s = createSurfaceStore();
    s.ingest(demoEnvelope);
    return s;
  }, []);

  const [kitName, setKitName] = useState<KitName>("shadcn");

  return (
    <div className="mx-auto max-w-md p-6 font-sans">
      <header className="mb-4 flex flex-col gap-1">
        <h1 className="text-xl font-bold">A2UI · 可插拔 ComponentKit 示例</h1>
        <p className="text-sm text-muted-foreground">
          同一套 A2UI 协议消息，切换 kit 即切换整个组件库；表单状态跨切换保留。
        </p>
      </header>

      <div
        role="tablist"
        aria-label="选择 ComponentKit"
        className="mb-4 inline-flex overflow-hidden rounded-md border"
      >
        {(Object.keys(KITS) as KitName[]).map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={k === kitName}
            onClick={() => setKitName(k)}
            className={
              k === kitName
                ? "bg-primary px-4 py-1.5 text-sm text-primary-foreground"
                : "px-4 py-1.5 text-sm hover:bg-accent"
            }
          >
            {KIT_LABEL[k]}
          </button>
        ))}
      </div>

      <p className="mb-3 text-xs text-muted-foreground">
        当前 kit：<code>{kitName}Kit</code>
      </p>

      <A2UIProvider store={store} kit={KITS[kitName]}>
        <Surface surfaceId="demo" />
      </A2UIProvider>
    </div>
  );
}
