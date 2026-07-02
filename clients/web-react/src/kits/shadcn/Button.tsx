import type { FC } from "react";

import { Button as ShadcnButton } from "@/components/ui/button";
import type { ButtonProps } from "@/contracts";

/** A2UI variant → shadcn button variant. */
const VARIANT_MAP = {
  primary: "default", // solid, bg-primary
  default: "secondary", // filled secondary
  borderless: "ghost", // no background / border
} as const;

/**
 * A2UI `Button` rendered with the shadcn `<Button>`.
 *
 * `onAction` is wired to `onClick`; `label` becomes the button children.
 */
export const Button: FC<ButtonProps> = ({
  label,
  variant,
  disabled,
  onAction,
}) => {
  return (
    <ShadcnButton
      variant={VARIANT_MAP[variant]}
      disabled={disabled}
      onClick={onAction}
    >
      {label}
    </ShadcnButton>
  );
};
