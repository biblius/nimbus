import {
  applyUploadFailure,
  pendingUploadPercent,
  type PendingUpload,
  type UploadHandlerOptions,
} from "@nimbus/shared/pending-upload";
import { session } from "@nimbus/shared/session";
import { toastError } from "@nimbus/shared/toast";
import type { NimbusNativeApi } from "./api";
import { listenNativeFileDrop } from "./native-file-drop";
import { pickNativeFiles, pickNativeFolder } from "./native-file-pick";
import type { NativeDropBatch, NativeUploadFile } from "./native-types";

/// How many files are sent at the same time.
const PARALLEL_UPLOADS = 10;

/**
 * Desktop uploads: filesystem paths streamed by Rust with byte progress.
 * OS pickers and Tauri drag-drop — not HTML5 `File` / XHR.
 */
export class UploadHandler {
  pending = $state<PendingUpload[]>([]);
  fileDragActive = $state(false);

  nextId = 0;
  controllers = new Map<number, AbortController>();
  opts: UploadHandlerOptions;

  constructor(opts: UploadHandlerOptions) {
    this.opts = opts;
  }

  percent(item: PendingUpload) {
    return pendingUploadPercent(item);
  }

  cancel(id: number) {
    this.controllers.get(id)?.abort();
    this.remove(id);
  }

  dismiss(id: number) {
    this.remove(id);
  }

  /** Native OS multi-file dialog. No-op if cancelled. */
  async pickFiles() {
    try {
      const files = await pickNativeFiles();

      if (!files) {
        return;
      }

      if (files.length === 0) {
        toastError("No files selected");
        return;
      }

      await this.uploadFiles(files);
    } catch (cause) {
      toastError(pickErrorMessage(cause, "Could not open the selected files"));
    }
  }

  /** Native OS folder dialog. No-op if cancelled. */
  async pickFolder() {
    try {
      const files = await pickNativeFolder();

      if (!files) {
        return;
      }

      if (files.length === 0) {
        toastError("That folder has no files to upload");
        return;
      }

      await this.uploadFolder(files);
    } catch (cause) {
      toastError(pickErrorMessage(cause, "Could not open the selected folder"));
    }
  }

  /**
   * Subscribe to Tauri OS file drops. Returns a disposer for `$effect` cleanup.
   * HTML5 drag events do not fire in the webview while native DnD is on.
   */
  attachDrop(canAccept: () => boolean): () => void {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listenNativeFileDrop((event) => {
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

      if (event.type === "error") {
        this.fileDragActive = false;
        toastError(event.message);
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
      uploads.push(this.uploadFolder(files));
    }

    await Promise.all(uploads);
  }

  async uploadFiles(files: NativeUploadFile[]) {
    if (files.length === 0) {
      return;
    }

    const client = session.client as NimbusNativeApi;
    const rootId = this.opts.rootId();
    const parent = this.opts.locationId();

    const queue = files.map((file) => {
      const id = this.nextId++;
      const controller = new AbortController();

      this.controllers.set(id, controller);

      this.pending.push({
        id,
        name: file.name.split("/").pop() || file.name,
        loaded: 0,
        total: file.size,
        error: "",
      });

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
            const created = await client.uploadPaths(
              rootId,
              parent,
              [file],
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

  async uploadFolder(files: NativeUploadFile[]) {
    if (files.length === 0) {
      return;
    }

    const client = session.client as NimbusNativeApi;
    const rootId = this.opts.rootId();
    const parent = this.opts.locationId();
    const id = this.nextId++;
    const controller = new AbortController();

    this.controllers.set(id, controller);

    const name = files[0]?.name.split("/")[0] || "Folder";
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
      const created = await client.uploadDirPaths(
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

  remove(id: number) {
    this.controllers.delete(id);

    const index = this.pending.findIndex((entry) => entry.id === id);

    if (index !== -1) {
      this.pending.splice(index, 1);
    }
  }

  failure(item: PendingUpload, cause: unknown) {
    this.controllers.delete(item.id);

    if (applyUploadFailure(item, cause)) {
      this.remove(item.id);
    }
  }
}

function pickErrorMessage(cause: unknown, fallback: string) {
  if (cause instanceof Error && cause.message) {
    return cause.message;
  }

  return fallback;
}
