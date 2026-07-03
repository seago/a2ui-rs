import { type FC, useId } from "react";

import { cn } from "@/lib/utils";
import type { CheckBoxProps } from "@/contracts";

/** A2UI `CheckBox`：复选框 + 可选 label，受控。 */
export const CheckBox: FC<CheckBoxProps> = ({
  checked,
  onChange,
  label,
  disabled,
}) => {
  const id = useId();
  return (
    <div data-slot="a2ui-checkbox" className={cn("flex items-center gap-2")}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className={cn(
          "size-4 rounded border-input accent-primary outline-none",
          "focus-visible:ring-ring/50 focus-visible:ring-[3px]",
          disabled && "opacity-50",
        )}
      />
      {label ? (
        <label htmlFor={id} className="text-sm leading-none select-none">
          {label}
        </label>
      ) : null}
    </div>
  );
};
