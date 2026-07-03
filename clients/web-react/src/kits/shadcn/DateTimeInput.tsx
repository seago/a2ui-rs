import { type FC, useId } from "react";

import { cn } from "@/lib/utils";
import type { DateTimeInputProps } from "@/contracts";

/** A2UI `DateTimeInput`：日期/时间/日期时间输入，受控。 */
export const DateTimeInput: FC<DateTimeInputProps> = ({
  value,
  onChange,
  label,
  enableDate,
  enableTime,
  min,
  max,
  disabled,
}) => {
  const id = useId();
  const type =
    enableDate && enableTime
      ? "datetime-local"
      : enableTime
        ? "time"
        : "date";
  return (
    <div data-slot="a2ui-datetime" className={cn("flex flex-col gap-1")}>
      {label ? (
        <label htmlFor={id} className="text-sm font-medium">
          {label}
        </label>
      ) : null}
      <input
        id={id}
        type={type}
        value={value}
        min={min}
        max={max}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className={cn(
          "h-9 rounded-md border border-input bg-background px-3 py-1 text-sm outline-none",
          "focus-visible:ring-ring/50 focus-visible:ring-[3px]",
          disabled && "opacity-50",
        )}
      />
    </div>
  );
};
