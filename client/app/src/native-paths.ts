import { basename, join } from "@tauri-apps/api/path";
import { readDir, readFile, stat } from "@tauri-apps/plugin-fs";
import type { NativeDropBatch } from "@nimbus/shared/session";

/**
 * Turn absolute filesystem paths into `File`s for the shared upload pipeline.
 * Directories are walked; members use `/`-separated relative `name`s.
 */
export async function filesFromPaths(paths: string[]): Promise<NativeDropBatch> {
  const files: File[] = [];
  const directories: File[][] = [];

  for (const path of paths) {
    const info = await stat(path);

    if (info.isDirectory) {
      const name = await basename(path);
      const dirFiles = await walkDirectory(path, name);

      if (dirFiles.length > 0) {
        directories.push(dirFiles);
      }
    } else if (info.isFile) {
      files.push(await fileFromPath(path));
    }
  }

  return { files, directories };
}

/** Read one directory tree into upload-ready `File`s (relative paths in `name`). */
export async function filesFromDirectory(dirPath: string): Promise<File[]> {
  const name = await basename(dirPath);
  return walkDirectory(dirPath, name);
}

async function walkDirectory(dirPath: string, prefix: string) {
  const entries = await readDir(dirPath);
  const files: File[] = [];

  for (const entry of entries) {
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

async function fileFromPath(path: string, relativeName?: string) {
  const bytes = await readFile(path);
  const name = relativeName ?? (await basename(path));
  // Ensure a plain ArrayBuffer-backed view for the File constructor.
  const copy = Uint8Array.from(bytes);

  return new File([copy], name);
}
