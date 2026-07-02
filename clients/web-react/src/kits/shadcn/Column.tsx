import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { ColumnProps } from "@/contracts";

/** A2UI `Column` 纵向布局容器（Tailwind flex-col）。 */
export const Column: FC<ColumnProps> = ({ children }) => {
  return (
    <div data-slot="a2ui-column" className={cn("flex flex-col gap-2")}>
      {children}
    </div>
  );
};
