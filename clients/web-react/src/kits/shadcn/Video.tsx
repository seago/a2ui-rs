import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { VideoProps } from "@/contracts";

/** A2UI `Video`：原生视频播放器（controls）。 */
export const Video: FC<VideoProps> = ({ url, posterUrl }) => (
  <video
    data-slot="a2ui-video"
    src={url}
    poster={posterUrl}
    controls
    className={cn("max-w-full rounded-md")}
  />
);
