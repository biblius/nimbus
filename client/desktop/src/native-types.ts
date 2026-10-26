/** Filesystem path ready for the desktop streaming upload command. */
export type NativeUploadFile = {
  path: string;
  /** Basename, or `/`-separated relative path for directory members. */
  name: string;
  size: number;
};

/** Files collected from a native (Tauri) OS drop. */
export type NativeDropBatch = {
  files: NativeUploadFile[];
  /** Each inner list is one dropped folder tree (relative paths in `name`). */
  directories: NativeUploadFile[][];
};

/** OS drag-drop lifecycle from the Tauri webview. */
export type NativeDropEvent =
  | { type: "enter" }
  | { type: "leave" }
  | { type: "drop"; batch: NativeDropBatch }
  | { type: "error"; message: string };
