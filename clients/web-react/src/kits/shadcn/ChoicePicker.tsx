import { type FC, useId } from "react";

import { cn } from "@/lib/utils";
import type { ChoicePickerProps } from "@/contracts";

/**
 * A2UI `ChoicePicker`：多选（multipleSelection）或互斥单选（mutuallyExclusive），
 * checkbox 或 chips 外观。受控 `value`（已选值数组）。
 */
export const ChoicePicker: FC<ChoicePickerProps> = ({
  value,
  onChange,
  options,
  variant,
  displayStyle,
  disabled,
}) => {
  const name = useId();
  const multiple = variant === "multipleSelection";

  const toggle = (v: string) => {
    if (multiple) {
      onChange(
        value.includes(v) ? value.filter((x) => x !== v) : [...value, v],
      );
    } else {
      onChange([v]);
    }
  };

  if (displayStyle === "chips") {
    return (
      <div
        data-slot="a2ui-choicepicker"
        role="group"
        className="flex flex-wrap gap-2"
      >
        {options.map((o) => {
          const selected = value.includes(o.value);
          return (
            <button
              key={o.value}
              type="button"
              aria-pressed={selected}
              disabled={disabled}
              onClick={() => toggle(o.value)}
              className={cn(
                "rounded-full border px-3 py-1 text-sm transition-colors",
                selected
                  ? "bg-primary text-primary-foreground border-primary"
                  : "bg-background hover:bg-accent",
                disabled && "pointer-events-none opacity-50",
              )}
            >
              {o.label}
            </button>
          );
        })}
      </div>
    );
  }

  return (
    <div
      data-slot="a2ui-choicepicker"
      role="group"
      className="flex flex-col gap-2"
    >
      {options.map((o) => {
        const selected = value.includes(o.value);
        return (
          <label key={o.value} className="flex items-center gap-2 text-sm">
            <input
              type={multiple ? "checkbox" : "radio"}
              name={multiple ? undefined : name}
              checked={selected}
              disabled={disabled}
              onChange={() => toggle(o.value)}
              className="size-4 accent-primary"
            />
            {o.label}
          </label>
        );
      })}
    </div>
  );
};
