import { type FC, useState } from "react";

import { cn } from "@/lib/utils";
import type { TabsProps } from "@/contracts";

/** A2UI `Tabs`：标签页容器，本地 state 记当前页（默认第 0 个）。 */
export const Tabs: FC<TabsProps> = ({ tabs }) => {
  const [active, setActive] = useState(0);
  return (
    <div data-slot="a2ui-tabs" className="flex flex-col gap-2">
      <div role="tablist" className="flex gap-1 border-b border-border">
        {tabs.map((t, i) => (
          <button
            key={i}
            role="tab"
            type="button"
            aria-selected={i === active}
            onClick={() => setActive(i)}
            className={cn(
              "-mb-px border-b-2 px-3 py-1.5 text-sm transition-colors",
              i === active
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            {t.title}
          </button>
        ))}
      </div>
      <div role="tabpanel" className="py-1">
        {tabs[active]?.content ?? null}
      </div>
    </div>
  );
};
