import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { TextProps } from "@/contracts";

/**
 * A2UI `Text` rendered with shadcn typography tokens.
 *
 * - `body` → **inherits** the surrounding color（不硬编码 `text-foreground`），
 *   这样作为按钮等有色容器的 label 时能正确继承对比色（如 primary 按钮上的白字），
 *   在普通容器里则继承默认前景色。
 * - `caption` → smaller, muted secondary text.
 */
export const Text: FC<TextProps> = ({ text, variant }) => {
  return (
    <p
      data-slot="a2ui-text"
      className={cn(
        variant === "caption" ? "text-xs text-muted-foreground" : "text-sm",
      )}
    >
      {text}
    </p>
  );
};
