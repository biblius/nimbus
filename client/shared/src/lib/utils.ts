import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Primary input is mouse-like (hover + fine pointer).
 * Used to prefer Ctrl/Shift multi-select over long-press.
 */
export function hasFinePointerHover() {
  return window.matchMedia("(hover: hover) and (pointer: fine)").matches;
}
