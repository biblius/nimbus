import { open } from "@tauri-apps/plugin-dialog";
import { filesFromDirectory, filesFromPaths } from "./native-paths";
import type { NativeUploadFile } from "./native-types";

/**
 * Native multi-file picker. Returns `null` if the user cancels.
 */
export async function pickNativeFiles(): Promise<NativeUploadFile[] | null> {
  const selected = await open({
    multiple: true,
    directory: false,
  });

  if (selected === null) {
    return null;
  }

  const paths = Array.isArray(selected) ? selected : [selected];
  const batch = await filesFromPaths(paths);

  return batch.files;
}

/**
 * Native folder picker. Returns the folder tree as path-prefixed descriptors,
 * or `null` if the user cancels. Empty folders yield an empty array.
 *
 * `recursive: true` tells the dialog plugin to grant FS scope for nested paths
 * so `readDir` / `stat` can walk the tree after the pick.
 */
export async function pickNativeFolder(): Promise<NativeUploadFile[] | null> {
  const selected = await open({
    multiple: false,
    directory: true,
    recursive: true,
  });

  if (selected === null) {
    return null;
  }

  const path = Array.isArray(selected) ? selected[0] : selected;

  if (!path) {
    return null;
  }

  return filesFromDirectory(path);
}
