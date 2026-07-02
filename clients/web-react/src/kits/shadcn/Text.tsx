import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { TextProps } from "@/contracts";

/**
 * A2UI `Text` rendered with shadcn typography tokens.
 *
 * - `body` → default foreground text.
 * - `caption` → smaller, muted secondary text.
 */
export const Text: FC<TextProps> = ({ text, variant }) => {
  return (
    <p
      data-slot="a2ui-text"
      className={cn(
        variant === "caption"
          ? "text-xs text-muted-foreground"
          : "text-sm text-foreground"
      )}
    >
      {text}
    </p>
  );
};
