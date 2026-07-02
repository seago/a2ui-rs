import { useId, type FC } from "react";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { TextFieldProps } from "@/contracts";

/** A2UI TextField variant → native input `type` (for the single-line case). */
const INPUT_TYPE: Record<string, string> = {
  shortText: "text",
  number: "number",
  obscured: "password",
};

/**
 * A2UI `TextField` rendered with shadcn `<Input>` / `<Textarea>`.
 *
 * - `longText` → `<Textarea>`; otherwise `<Input>` with a mapped `type`
 *   (`obscured` → password, `number` → number, `shortText` → text).
 * - Controlled via `value` + `onChange(rawString)`.
 * - Renders an associated `<Label>` when `label` is provided.
 * - When `errors` is non-empty, shows the destructive error state and the
 *   first error message.
 */
export const TextField: FC<TextFieldProps> = ({
  value,
  onChange,
  label,
  placeholder,
  variant,
  disabled,
  errors,
}) => {
  const id = useId();
  const hasErrors = errors.length > 0;
  const errorId = `${id}-error`;

  const common = {
    id,
    value,
    placeholder,
    disabled,
    "aria-invalid": hasErrors || undefined,
    "aria-describedby": hasErrors ? errorId : undefined,
    onChange: (
      e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>
    ) => onChange(e.target.value),
  };

  const control =
    variant === "longText" ? (
      <Textarea {...common} />
    ) : (
      <Input type={INPUT_TYPE[variant] ?? "text"} {...common} />
    );

  return (
    <div className={cn("flex flex-col gap-1.5")} data-slot="a2ui-textfield">
      {label ? <Label htmlFor={id}>{label}</Label> : null}
      {control}
      {hasErrors ? (
        <p id={errorId} className="text-xs text-destructive">
          {errors[0].message}
        </p>
      ) : null}
    </div>
  );
};
