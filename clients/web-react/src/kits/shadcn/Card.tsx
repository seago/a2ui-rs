import type { FC } from "react";

import { Card as ShadcnCard, CardContent } from "@/components/ui/card";
import type { CardProps } from "@/contracts";

/** A2UI `Card` container rendered with the shadcn `<Card>`. */
export const Card: FC<CardProps> = ({ children }) => {
  return (
    <ShadcnCard data-slot="a2ui-card">
      <CardContent>{children}</CardContent>
    </ShadcnCard>
  );
};
