import { open } from "@tauri-apps/plugin-dialog";
import { filesFromDirectory, filesFromPaths } from "./native-paths";

/**
 * Native multi-file picker. Returns `null` if the user cancels.
 */
export async function pickNativeFiles(): Promise<File[] | null> {
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
 * Native folder picker. Returns the folder tree as path-prefixed `File`s,
 * or `null` if the user cancels. Empty folders yield an empty array.
 */
export async function pickNativeFolder(): Promise<File[] | null> {
  const selected = await open({
    multiple: false,
    directory: true,
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
