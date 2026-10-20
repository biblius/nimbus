import type { FileType } from "./types";

/** Listing row with the fields preview helpers need. */
type FileLike = {
  file: {
    kind: "file" | "directory";
    fileType: FileType | null;
  };
};

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

export function parsePreviewFilter(
  value: string | null | undefined,
): PreviewFilter {
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

export function isImageFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" && fileFamily(entry.file.fileType) === "image"
  );
}

export function isVideoFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" && fileFamily(entry.file.fileType) === "video"
  );
}

export function isAudioFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" && fileFamily(entry.file.fileType) === "audio"
  );
}

export function isTextFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" && fileFamily(entry.file.fileType) === "text"
  );
}

export function isPdfFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" &&
    entry.file.fileType != null &&
    "doc" in entry.file.fileType &&
    entry.file.fileType.doc === "Pdf"
  );
}

/**
 * Spreadsheet formats SheetJS can preview that we already classify.
 * @see https://docs.sheetjs.com/docs/miscellany/formats
 *
 * Included: Xls / Xlsx / Ods (doc), Csv (text), Rtf (archive).
 * Html / Xml / Txt are also on the SheetJS list but stay as text preview —
 * those variants are general documents, not Excel worksheet flavours.
 */
export function isXlsxFile(entry: FileLike): boolean {
  if (entry.file.kind !== "file" || entry.file.fileType == null) {
    return false;
  }

  const fileType = entry.file.fileType;

  if ("doc" in fileType) {
    return (
      fileType.doc === "Xlsx" ||
      fileType.doc === "Xls" ||
      fileType.doc === "Ods"
    );
  }

  if ("text" in fileType) {
    return fileType.text === "Csv";
  }

  if ("archive" in fileType) {
    return fileType.archive === "Rtf";
  }

  return false;
}

/** ZIP archives — listed via SheetJS `CFB.read` (same lib as xlsx preview). */
export function isZipFile(entry: FileLike): boolean {
  return (
    entry.file.kind === "file" &&
    entry.file.fileType != null &&
    "archive" in entry.file.fileType &&
    entry.file.fileType.archive === "Zip"
  );
}

/** Files that show a generated WebP icon in the browser list. */
export function hasListThumbnail(entry: FileLike): boolean {
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

export function isPreviewable(entry: FileLike): boolean {
  return (
    isImageFile(entry) ||
    isVideoFile(entry) ||
    isAudioFile(entry) ||
    isTextFile(entry) ||
    isPdfFile(entry) ||
    isXlsxFile(entry) ||
    isZipFile(entry)
  );
}

const FILE_ICON_COLORS = {
  pdf: "oklch(0.58 0.21 29)",
  doc: "oklch(0.66 0.13 235)",
  sheet: "oklch(0.68 0.14 138)",
  slide: "oklch(0.74 0.12 55)",
  image: "oklch(0.58 0.2 325)",
  video: "oklch(0.58 0.2 29)",
  audio: "oklch(0.72 0.15 70)",
  text: "oklch(0.55 0.1 265)",
  archive: "oklch(0.58 0.08 70)",
  book: "oklch(0.55 0.14 305)",
  app: "oklch(0.58 0.12 240)",
  font: "oklch(0.55 0.03 250)",
  other: "oklch(0.55 0.02 260)",
} as const;

/** Foreground color for a file-type Lucide icon (`currentColor`). */
export function fileIconColor(fileType: FileType | null | undefined): string {
  if (fileType == null) {
    return FILE_ICON_COLORS.other;
  }

  if ("doc" in fileType) {
    switch (fileType.doc) {
      case "Pdf":
        return FILE_ICON_COLORS.pdf;
      case "Xls":
      case "Xlsx":
      case "Ods":
        return FILE_ICON_COLORS.sheet;
      case "Ppt":
      case "Pptx":
      case "Odp":
        return FILE_ICON_COLORS.slide;
      case "Doc":
      case "Docx":
      case "Odt":
        return FILE_ICON_COLORS.doc;
    }
  }

  if ("text" in fileType && fileType.text === "Csv") {
    return FILE_ICON_COLORS.sheet;
  }

  switch (fileFamily(fileType)) {
    case "image":
      return FILE_ICON_COLORS.image;
    case "video":
      return FILE_ICON_COLORS.video;
    case "audio":
      return FILE_ICON_COLORS.audio;
    case "text":
      return FILE_ICON_COLORS.text;
    case "archive":
      return FILE_ICON_COLORS.archive;
    case "book":
      return FILE_ICON_COLORS.book;
    case "app":
      return FILE_ICON_COLORS.app;
    case "font":
      return FILE_ICON_COLORS.font;
    case "doc":
      return FILE_ICON_COLORS.doc;
    default:
      return FILE_ICON_COLORS.other;
  }
}

/** Inline style setting `--file-icon` for tinted chips. */
export function fileIconChipStyle(
  fileType: FileType | null | undefined,
): string {
  return `--file-icon: ${fileIconColor(fileType)}`;
}

/**
 * Chip surface using `color-mix` + `--file-icon` (set via {@link fileIconChipStyle}).
 * Pair with ring utilities already on the host.
 */
export const FILE_ICON_CHIP_CLASS =
  "bg-[color-mix(in_oklab,var(--file-icon)_14%,transparent)] text-[color:var(--file-icon)] ring-[color-mix(in_oklab,var(--file-icon)_28%,transparent)]";
