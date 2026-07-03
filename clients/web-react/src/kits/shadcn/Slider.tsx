import { type FC, useId } from "react";

import { cn } from "@/lib/utils";
import type { SliderProps } from "@/contracts";

/** A2UI `Slider`：滑块（原生 range），受控，onChange 传 number。 */
export const Slider: FC<SliderProps> = ({
  value,
  onChange,
  min,
  max,
  step,
  label,
  disabled,
}) => {
  const id = useId();
  return (
    <div data-slot="a2ui-slider" className={cn("flex flex-col gap-1")}>
      {label ? (
        <label htmlFor={id} className="flex justify-between text-sm">
          <span>{label}</span>
          <span className="text-muted-foreground">{value}</span>
        </label>
      ) : null}
      <input
        id={id}
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        className={cn("w-full accent-primary", disabled && "opacity-50")}
      />
    </div>
  );
};
