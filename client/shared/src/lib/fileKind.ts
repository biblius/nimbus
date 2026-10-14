import type { FileType, FileView } from "./types";

/** Top-level file family from the API `fileType` object. */
export type FileFamily =
  | "app"
  | "book"
  | "image"
  | "video"
  | "audio"
  | "font"
  | "doc"
  | "archive"
  | "text"
  | "other";

export function fileFamily(fileType: FileType | null | undefined): FileFamily {
  if (fileType == null) {
    return "other";
  }
  if ("app" in fileType) {
    return "app";
  }
  if ("book" in fileType) {
    return "book";
  }
  if ("image" in fileType) {
    return "image";
  }
  if ("video" in fileType) {
    return "video";
  }
  if ("audio" in fileType) {
    return "audio";
  }
  if ("font" in fileType) {
    return "font";
  }
  if ("doc" in fileType) {
    return "doc";
  }
  if ("archive" in fileType) {
    return "archive";
  }
  if ("text" in fileType) {
    return "text";
  }
  return "other";
}

export function isImageFile(entry: FileView): boolean {
  return entry.file.kind === "file" && fileFamily(entry.file.fileType) === "image";
}

export function isTextFile(entry: FileView): boolean {
  return entry.file.kind === "file" && fileFamily(entry.file.fileType) === "text";
}

export function isPreviewable(entry: FileView): boolean {
  return isImageFile(entry) || isTextFile(entry);
}
