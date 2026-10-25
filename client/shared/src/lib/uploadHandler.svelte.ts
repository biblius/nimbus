import { ApiError } from "./api-errors";
import {
  session,
  type NativeDropBatch,
  type NativeDropEvent,
} from "./session.svelte";
import type { FileEntry } from "./types";

/// How many files are sent at the same time.
const PARALLEL_UPLOADS = 10;

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

/**
 * Queued file/folder uploads with progress, abort, and OS drag-drop.
 * One instance per FileBrowser mount.
 */
export class UploadHandler {
  pending = $state<PendingUpload[]>([]);
  fileDragActive = $state(false);

  nextId = 0;
  controllers = new Map<number, AbortController>();
  fileDragDepth = 0;
  opts: UploadHandlerOptions;

  constructor(opts: UploadHandlerOptions) {
    this.opts = opts;
  }

  percent(item: PendingUpload) {
    if (item.total <= 0) {
      return 0;
    }
    return Math.min(100, Math.round((item.loaded / item.total) * 100));
  }

  cancel(id: number) {
    this.controllers.get(id)?.abort();
    this.remove(id);
  }

  /** Drop a finished/failed row from the list (no abort). */
  dismiss(id: number) {
    this.remove(id);
  }

  async fromFileInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files: File[] = [];

    if (input.files) {
      for (const file of input.files) {
        files.push(file);
      }
    }

    input.value = "";
    await this.uploadFiles(files);
  }

  async fromFolderInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files: File[] = [];

    if (input.files) {
      for (const file of input.files) {
        files.push(file);
      }
    }

    input.value = "";
    await this.uploadFolderFiles(files);
  }

  /** Native OS multi-file dialog (Tauri). No-op if cancelled or unavailable. */
  async fromNativeFilePicker() {
    const files = await session.pickFiles?.();

    if (!files) {
      return;
    }

    await this.uploadFiles(files);
  }

  /** Native OS folder dialog (Tauri). No-op if cancelled or unavailable. */
  async fromNativeFolderPicker() {
    const files = await session.pickFolder?.();

    if (!files) {
      return;
    }

    await this.uploadFolderFiles(files);
  }

  async uploadFiles(files: File[]) {
    if (files.length === 0) {
      return;
    }

    const rootId = this.opts.rootId();
    const parent = this.opts.locationId();

    const queue = files.map((file) => {
      const id = this.nextId++;
      const controller = new AbortController();

      this.controllers.set(id, controller);

      this.pending.push({
        id,
        name: file.name,
        loaded: 0,
        total: file.size,
        error: "",
      });

      // Read the element back so later writes go through the reactive proxy.
      const item = this.pending[this.pending.length - 1];

      return { file, item, controller };
    });

    const workers = Array.from(
      { length: Math.min(PARALLEL_UPLOADS, queue.length) },
      async () => {
        while (queue.length > 0) {
          const { file, item, controller } = queue.shift()!;

          if (controller.signal.aborted) {
            continue;
          }

          try {
            const created = await session.client.upload(
              rootId,
              parent,
              file,
              (loaded, total) => {
                item.loaded = loaded;
                item.total = total;
              },
              controller.signal,
            );

            this.remove(item.id);

            if (
              this.opts.rootId() === rootId &&
              this.opts.locationId() === parent
            ) {
              this.opts.onCreated(created);
            }
          } catch (cause) {
            this.failure(item, cause);
          }
        }
      },
    );

    await Promise.all(workers);
  }

  async uploadFolderFiles(files: File[]) {
    if (files.length === 0) {
      return;
    }

    const rootId = this.opts.rootId();
    const parent = this.opts.locationId();
    const id = this.nextId++;
    const controller = new AbortController();

    this.controllers.set(id, controller);

    const path = files[0]?.webkitRelativePath || files[0]?.name || "Folder";
    const name = path.split("/")[0] || "Folder";
    const total = files.reduce((sum, file) => sum + file.size, 0);

    this.pending.push({
      id,
      name,
      loaded: 0,
      total,
      error: "",
    });

    const item = this.pending[this.pending.length - 1];

    try {
      const created = await session.client.uploadDir(
        rootId,
        parent,
        files,
        (loaded, totalBytes) => {
          item.loaded = loaded;
          item.total = totalBytes;
        },
        controller.signal,
      );

      this.remove(item.id);

      if (
        this.opts.rootId() === rootId &&
        this.opts.locationId() === parent
      ) {
        for (const entry of created) {
          this.opts.onCreated(entry);
        }
      }
    } catch (cause) {
      this.failure(item, cause);
    }
  }

  onWindowDragEnter(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    this.fileDragDepth += 1;
    this.fileDragActive = true;
  }

  onWindowDragLeave(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    this.fileDragDepth = Math.max(0, this.fileDragDepth - 1);

    if (this.fileDragDepth === 0) {
      this.fileDragActive = false;
    }
  }

  onWindowDragOver(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "copy";
    }
  }

  async onWindowDrop(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    this.fileDragDepth = 0;
    this.fileDragActive = false;

    const batch = await collectDropped(event.dataTransfer);
    await this.fromDropBatch(batch);
  }

  /**
   * Subscribe to Tauri OS file drops. Returns a disposer for `$effect` cleanup.
   * HTML5 drag events do not fire in the webview while native DnD is on.
   */
  attachNativeDrop(canAccept: () => boolean): () => void {
    const listen = session.listenFileDrop;

    if (!listen) {
      return () => {};
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen((event: NativeDropEvent) => {
      if (!canAccept()) {
        this.fileDragActive = false;
        return;
      }

      if (event.type === "enter") {
        this.fileDragActive = true;
        return;
      }

      if (event.type === "leave") {
        this.fileDragActive = false;
        return;
      }

      this.fileDragActive = false;
      void this.fromDropBatch(event.batch);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }

      unlisten = fn;
    });

    return () => {
      disposed = true;
      unlisten?.();
      this.fileDragActive = false;
    };
  }

  async fromDropBatch(batch: NativeDropBatch) {
    const uploads = [this.uploadFiles(batch.files)];

    for (const files of batch.directories) {
      uploads.push(this.uploadFolderFiles(files));
    }

    await Promise.all(uploads);
  }

  remove(id: number) {
    this.controllers.delete(id);

    const index = this.pending.findIndex((entry) => entry.id === id);

    if (index !== -1) {
      this.pending.splice(index, 1);
    }
  }

  failure(item: PendingUpload, cause: unknown) {
    this.controllers.delete(item.id);

    if (cause instanceof DOMException && cause.name === "AbortError") {
      this.remove(item.id);
      return;
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
  }
}

function isExternalFileDrag(event: DragEvent) {
  return Boolean(event.dataTransfer?.types.includes("Files"));
}

function supportsExternalFileDrop() {
  return window.matchMedia("(pointer: fine)").matches;
}

/**
 * Directory members are `File`s whose `name` is the relative path so
 * `uploadDir` can recreate the tree via `webkitRelativePath || name`.
 */
async function collectDropped(dataTransfer: DataTransfer | null) {
  const files: File[] = [];
  const directories: File[][] = [];

  if (!dataTransfer) {
    return { files, directories };
  }

  const items = dataTransfer.items;

  if (
    items &&
    items.length > 0 &&
    typeof items[0].webkitGetAsEntry === "function"
  ) {
    for (let i = 0; i < items.length; i++) {
      const entry = items[i].webkitGetAsEntry();
      if (!entry) {
        continue;
      }

      if (entry.isFile) {
        files.push(await readFileEntry(entry as FileSystemFileEntry));
      } else if (entry.isDirectory) {
        const dirFiles = await readDirectoryEntry(
          entry as FileSystemDirectoryEntry,
          entry.name,
        );
        if (dirFiles.length > 0) {
          directories.push(dirFiles);
        }
      }
    }

    return { files, directories };
  }

  for (let i = 0; i < dataTransfer.files.length; i++) {
    files.push(dataTransfer.files[i]);
  }

  return { files, directories };
}

function readFileEntry(entry: FileSystemFileEntry) {
  return new Promise<File>((resolve, reject) => {
    entry.file(resolve, reject);
  });
}

function readAllEntries(reader: FileSystemDirectoryReader) {
  return new Promise<FileSystemEntry[]>((resolve, reject) => {
    const all: FileSystemEntry[] = [];

    const readBatch = () => {
      reader.readEntries((batch) => {
        if (batch.length === 0) {
          resolve(all);
          return;
        }
        for (const entry of batch) {
          all.push(entry);
        }
        readBatch();
      }, reject);
    };

    readBatch();
  });
}

async function readDirectoryEntry(
  entry: FileSystemDirectoryEntry,
  prefix: string,
) {
  const children = await readAllEntries(entry.createReader());
  const files: File[] = [];

  for (const child of children) {
    if (child.isFile) {
      const file = await readFileEntry(child as FileSystemFileEntry);

      files.push(new File([file], `${prefix}/${file.name}`));
    } else if (child.isDirectory) {
      const nested = await readDirectoryEntry(
        child as FileSystemDirectoryEntry,
        `${prefix}/${child.name}`,
      );

      for (const file of nested) {
        files.push(file);
      }
    }
  }

  return files;
}
