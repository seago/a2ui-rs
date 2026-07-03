import type { FC } from "react";
import {
  Home,
  Search,
  Check,
  X,
  User,
  Settings,
  Heart,
  Star,
  Menu,
  ArrowRight,
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  Plus,
  Minus,
  Trash2,
  Pencil,
  Mail,
  Calendar,
  Clock,
  Info,
  AlertCircle,
  HelpCircle,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { IconProps } from "@/contracts";

/** name → lucide 图标映射（覆盖常见图标；未知名回退 HelpCircle）。 */
const ICONS: Record<string, LucideIcon> = {
  home: Home,
  search: Search,
  check: Check,
  close: X,
  x: X,
  user: User,
  settings: Settings,
  heart: Heart,
  star: Star,
  menu: Menu,
  "arrow-right": ArrowRight,
  "arrow-left": ArrowLeft,
  "chevron-down": ChevronDown,
  "chevron-right": ChevronRight,
  add: Plus,
  plus: Plus,
  minus: Minus,
  delete: Trash2,
  trash: Trash2,
  edit: Pencil,
  mail: Mail,
  email: Mail,
  calendar: Calendar,
  clock: Clock,
  info: Info,
  warning: AlertCircle,
  help: HelpCircle,
};

/** A2UI `Icon`：按 name 渲染 lucide 矢量图标，未知名回退 HelpCircle。 */
export const Icon: FC<IconProps> = ({ name }) => {
  const Cmp = ICONS[name] ?? HelpCircle;
  return (
    <Cmp
      data-slot="a2ui-icon"
      role="img"
      aria-label={name}
      className={cn("size-4")}
    />
  );
};
