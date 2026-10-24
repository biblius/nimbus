import type { Root } from "../types";

export type DestinationRequest = {
  roots: Root[];
  title: string;
  /** Defaults to "Choose a folder". */
  description?: string;
  confirmLabel: string;
  /** Defaults to "Could not load folders". */
  loadError?: string;
  onConfirm: (root: Root, parent: string | null) => void | Promise<void>;
};
