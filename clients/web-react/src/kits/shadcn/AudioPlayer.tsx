import type { FC } from "react";

import { cn } from "@/lib/utils";
import type { AudioPlayerProps } from "@/contracts";

/** A2UI `AudioPlayer`：原生音频播放器（controls）+ 可选描述。 */
export const AudioPlayer: FC<AudioPlayerProps> = ({ url, description }) => (
  <div data-slot="a2ui-audio" className={cn("flex flex-col gap-1")}>
    <audio src={url} controls className="w-full" />
    {description ? (
      <span className="text-xs text-muted-foreground">{description}</span>
    ) : null}
  </div>
);
