import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { ImageProps } from "@/contracts";

const FIT_CLASS: Record<string, string> = {
  contain: "object-contain",
  cover: "object-cover",
  fill: "object-fill",
};

/** A2UI `Image`：图片展示，圆角 + object-fit（默认 cover）。 */
export const Image: FC<ImageProps> = ({ url, fit = "cover", variant }) => (
  <img
    data-slot="a2ui-image"
    src={url}
    alt={variant ?? "image"}
    className={cn("max-w-full rounded-md", FIT_CLASS[fit] ?? "object-cover")}
  />
);
