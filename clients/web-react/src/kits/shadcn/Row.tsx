import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { RowProps } from "@/contracts";

/** A2UI `Row` 横向布局容器（Tailwind flex-row）。 */
export const Row: FC<RowProps> = ({ children }) => {
  return (
    <div
      data-slot="a2ui-row"
      className={cn("flex flex-row items-center gap-2")}
    >
      {children}
    </div>
  );
};
