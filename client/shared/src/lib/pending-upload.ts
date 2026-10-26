import { ApiError } from "./api-errors";
import type { FileEntry } from "./types";

/** One row in the in-browser upload progress list. */
export type PendingUpload = {
  id: number;
  name: string;
  loaded: number;
  total: number;
  error: string;
};

export type UploadHandlerOptions = {
  rootId: () => string;
  locationId: () => string | null;
  onCreated: (entry: FileEntry) => void;
};

export function pendingUploadPercent(item: PendingUpload) {
  if (item.total <= 0) {
    return 0;
  }
  return Math.min(100, Math.round((item.loaded / item.total) * 100));
}

/**
 * Map an upload failure onto `item.error`.
 * Returns `true` when the upload was aborted (row should be removed).
 */
export function applyUploadFailure(item: PendingUpload, cause: unknown): boolean {
  if (cause instanceof DOMException && cause.name === "AbortError") {
    return true;
  }

  if (cause instanceof TypeError) {
    item.error = "The server could not be reached";
  } else if (cause instanceof ApiError && cause.status === 413) {
    item.error = "Not enough space";
  } else if (cause instanceof ApiError && cause.status === 403) {
    item.error = "You cannot upload here";
  } else {
    item.error = "Upload failed";
  }

  return false;
}
