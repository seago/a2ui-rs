import { type FC, useState } from "react";

import { cn } from "@/lib/utils";
import type { ModalProps } from "@/contracts";

/** A2UI `Modal`：点击 trigger 打开覆盖层显示 content，点遮罩/关闭按钮关闭。 */
export const Modal: FC<ModalProps> = ({ content, trigger }) => {
  const [open, setOpen] = useState(false);
  return (
    <>
      <span
        data-slot="a2ui-modal-trigger"
        onClick={() => setOpen(true)}
        className="inline-flex cursor-pointer"
      >
        {trigger}
      </span>
      {open ? (
        <div
          data-slot="a2ui-modal"
          role="dialog"
          aria-modal="true"
          onClick={() => setOpen(false)}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className={cn(
              "relative max-w-md rounded-lg border bg-background p-6 shadow-lg",
            )}
          >
            <button
              type="button"
              aria-label="关闭"
              onClick={() => setOpen(false)}
              className="absolute right-3 top-3 text-muted-foreground hover:text-foreground"
            >
              ×
            </button>
            {content}
          </div>
        </div>
      ) : null}
    </>
  );
};
