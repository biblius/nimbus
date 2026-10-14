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

/** Preview carousel filter: all families or one family. */
export type PreviewFilter = "all" | FileFamily;

export const FILE_FAMILY_LABELS: Record<FileFamily, string> = {
  app: "App",
  book: "Book",
  image: "Image",
  video: "Video",
  audio: "Audio",
  font: "Font",
  doc: "Document",
  archive: "Archive",
  text: "Text",
  other: "Other",
};

const FILE_FAMILIES = new Set<string>(Object.keys(FILE_FAMILY_LABELS));

export function parsePreviewFilter(value: string | null | undefined): PreviewFilter {
  if (value == null || value === "all") {
    return "all";
  }
  if (FILE_FAMILIES.has(value)) {
    return value as FileFamily;
  }
  return "all";
}

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

export function isVideoFile(entry: FileView): boolean {
  return entry.file.kind === "file" && fileFamily(entry.file.fileType) === "video";
}

export function isTextFile(entry: FileView): boolean {
  return entry.file.kind === "file" && fileFamily(entry.file.fileType) === "text";
}

/** Files that show a generated WebP icon in the browser list. */
export function hasListThumbnail(entry: FileView): boolean {
  return isImageFile(entry) || isVideoFile(entry);
}

/** Tree nodes (live or trash) that can show a generated WebP thumbnail. */
export function treeHasThumbnail(
  kind: "file" | "directory",
  fileType: FileType | null | undefined,
): boolean {
  if (kind !== "file") {
    return false;
  }
  const family = fileFamily(fileType);
  return family === "image" || family === "video";
}

export function isPreviewable(entry: FileView): boolean {
  return isImageFile(entry) || isVideoFile(entry) || isTextFile(entry);
}
