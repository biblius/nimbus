import { basename, join } from "@tauri-apps/api/path";
import { readDir, stat } from "@tauri-apps/plugin-fs";
import type { NativeDropBatch, NativeUploadFile } from "./native-types";

/**
 * Turn absolute filesystem paths into upload descriptors (path + relative name).
 * Directories are walked; members use `/`-separated relative `name`s.
 * Files are not read into memory — the Rust upload command streams from disk.
 *
 * Dotfiles / dot-directories (`.git`, `.env`, …) are skipped: on Unix Tauri's
 * default FS scope does not match leading-dot path segments under `$HOME/**`.
 */
export async function filesFromPaths(paths: string[]): Promise<NativeDropBatch> {
  const files: NativeUploadFile[] = [];
  const directories: NativeUploadFile[][] = [];

  for (const path of paths) {
    const info = await stat(path);

    if (info.isDirectory) {
      const name = await basename(path);

      if (isHiddenName(name)) {
        continue;
      }

      const dirFiles = await walkDirectory(path, name);

      if (dirFiles.length > 0) {
        directories.push(dirFiles);
      }
    } else if (info.isFile) {
      const name = await basename(path);

      if (isHiddenName(name)) {
        continue;
      }

      files.push(await fileFromPath(path, name));
    }
  }

  return { files, directories };
}

/** Read one directory tree into upload descriptors (relative paths in `name`). */
export async function filesFromDirectory(
  dirPath: string,
): Promise<NativeUploadFile[]> {
  const name = await basename(dirPath);
  return walkDirectory(dirPath, name);
}

async function walkDirectory(dirPath: string, prefix: string) {
  const entries = await readDir(dirPath);
  const files: NativeUploadFile[] = [];

  for (const entry of entries) {
    if (isHiddenName(entry.name)) {
      continue;
    }

    const childPath = await join(dirPath, entry.name);

    if (entry.isDirectory) {
      const nested = await walkDirectory(childPath, `${prefix}/${entry.name}`);

      for (const file of nested) {
        files.push(file);
      }
    } else if (entry.isFile) {
      files.push(await fileFromPath(childPath, `${prefix}/${entry.name}`));
    }
  }

  return files;
}

async function fileFromPath(
  path: string,
  relativeName?: string,
): Promise<NativeUploadFile> {
  const info = await stat(path);
  const name = relativeName ?? (await basename(path));

  return {
    path,
    name,
    size: info.size,
  };
}

/** True for `.git`, `.env`, etc. (not `.` / `..`). */
function isHiddenName(name: string) {
  return name.startsWith(".") && name !== "." && name !== "..";
}
