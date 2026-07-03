import type { FC } from "react";

import { Separator } from "@/components/ui/separator";
import type { DividerProps } from "@/contracts";

/** A2UI `Divider`：分割线（shadcn Separator）。 */
export const Divider: FC<DividerProps> = () => <Separator className="my-2" />;
