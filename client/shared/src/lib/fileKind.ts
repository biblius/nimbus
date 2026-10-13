import type { FileView } from "./types";

export function isImageFile(entry: FileView): boolean {
  return (
    entry.file.kind === "file" &&
    entry.file.fileType != null &&
    "image" in entry.file.fileType
  );
}

export function isTextFile(entry: FileView): boolean {
  return (
    entry.file.kind === "file" &&
    entry.file.fileType != null &&
    "text" in entry.file.fileType
  );
}

export function isPreviewable(entry: FileView): boolean {
  return isImageFile(entry) || isTextFile(entry);
}
