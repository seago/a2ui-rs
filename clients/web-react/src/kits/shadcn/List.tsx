import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { ListProps } from "@/contracts";

/** A2UI `List`：列表容器，按 direction 纵向/横向排列。 */
export const List: FC<ListProps> = ({ children, direction }) => (
  <div
    data-slot="a2ui-list"
    role="list"
    className={cn(
      "flex gap-2",
      direction === "horizontal" ? "flex-row" : "flex-col",
    )}
  >
    {children}
  </div>
);
