import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { PlaceholderProps } from "@/contracts";

/**
 * Fallback for unknown component types / missing references.
 *
 * Rendered as a conspicuous dashed, warning-colored block showing `reason`.
 */
export const Placeholder: FC<PlaceholderProps> = ({ reason }) => {
  return (
    <div
      role="note"
      data-slot="a2ui-placeholder"
      className={cn(
        "rounded-md border-2 border-dashed border-destructive/60",
        "bg-destructive/10 px-3 py-2 text-sm text-destructive"
      )}
    >
      {reason}
    </div>
  );
};
