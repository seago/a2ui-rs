import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge Tailwind class names, resolving conflicts (later classes win).
 *
 * @example
 * cn("px-2", "px-4") // => "px-4"
 * cn("text-sm", condition && "font-bold")
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
