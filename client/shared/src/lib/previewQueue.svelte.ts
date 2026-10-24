import { hasListThumbnail } from "./fileKind";
import { session } from "./session.svelte";
import type { FileType } from "./types";

/** Minimal fields needed to decide whether to fetch a list thumbnail. */
type PreviewFile = {
  id: string;
  kind: "file" | "directory";
  parent: string | null;
  fileType: FileType | null;
};

const PREVIEW_POLL_MS = 3000;
const PREVIEW_MAX_ATTEMPTS = 5;
/** Cap parallel icon fetches so a large folder does not stampede the server. */
const PARALLEL_FETCHES = 8;

type PreviewJob = {
  id: string;
  attempts: number;
  /** When true, failed fetches are retried (new uploads with a pending preview). */
  retry: boolean;
};

export type PreviewQueueOptions = {
  rootId: () => string;
  locationId: () => string | null;
  /** Preview pixel size: 64 for list icons, 256 for tiles. */
  getSize: () => number;
  /** Override authenticated preview fetch (e.g. public shared links). */
  fetchPreview?: (fileId: string, size: 64 | 256) => Promise<Blob>;
};

/**
 * Polls for list/tile thumbnails after upload or view-mode changes.
 * One instance per FileBrowser mount.
 */
export class PreviewQueue {
  thumbs = $state<Record<string, string>>({});

  queue: PreviewJob[] = [];
  timer: ReturnType<typeof setInterval> | undefined;
  polling = false;
  opts: PreviewQueueOptions;

  constructor(opts: PreviewQueueOptions) {
    this.opts = opts;
  }

  /**
   * Enqueue a thumbnail fetch.
   * @param retry - Only for new uploads where a preview is known to be pending.
   *   Existing files should leave this false (one attempt, no poll retries).
   */
  enqueue(file: PreviewFile, retry = false) {
    if (this.tryQueue(file, retry)) {
      void this.drainQueue();
    }

    if (retry && this.timer === undefined) {
      this.setPollInterval();
    }
  }

  /** Queue many files then drain once (avoids starting a 1-job drain mid-loop). */
  enqueueAll(files: PreviewFile[], retry = false) {
    let queued = false;

    for (const file of files) {
      if (this.tryQueue(file, retry)) {
        queued = true;
      }
    }

    if (queued) {
      void this.drainQueue();
    }

    if (retry && this.timer === undefined) {
      this.setPollInterval();
    }
  }

  /**
   * Revoke cached thumbs and re-enqueue existing files (no retry).
   */
  refreshAll(files: PreviewFile[]) {
    for (const url of Object.values(this.thumbs)) {
      URL.revokeObjectURL(url);
    }

    this.thumbs = {};
    this.enqueueAll(files, false);
  }

  /**
   * Stop polling, revoke all object URLs and reset the state.
   */
  clear() {
    this.clearPollInterval();

    this.queue.length = 0;

    for (const url of Object.values(this.thumbs)) {
      URL.revokeObjectURL(url);
    }

    this.thumbs = {};
  }

  /** @returns true when a new job was pushed. */
  private tryQueue(file: PreviewFile, retry: boolean): boolean {
    if (file.kind !== "file" || file.parent !== this.opts.locationId()) {
      return false;
    }

    if (!hasListThumbnail({ file, metadata: null })) {
      return false;
    }

    if (this.thumbs[file.id] || this.queue.some((job) => job.id === file.id)) {
      return false;
    }

    this.queue.push({ id: file.id, attempts: 0, retry });
    return true;
  }

  private clearPollInterval() {
    if (this.timer !== undefined) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
  }

  private setPollInterval() {
    this.timer = setInterval(() => {
      void this.drainQueue();
    }, PREVIEW_POLL_MS);
  }

  private async drainQueue() {
    if (this.polling || this.queue.length === 0) {
      if (this.queue.length === 0) {
        this.clearPollInterval();
      }
      return;
    }

    this.polling = true;

    const rootId = this.opts.rootId();
    const previewSize = this.opts.getSize();
    const jobs = this.queue.slice(0, PARALLEL_FETCHES);

    try {
      await Promise.all(
        jobs.map(async (job) => {
          try {
            const size = (previewSize === 256 ? 256 : 64) as 64 | 256;
            const blob = this.opts.fetchPreview
              ? await this.opts.fetchPreview(job.id, size)
              : await session.client.icon(rootId, job.id, size);

            const prev = this.thumbs[job.id];

            if (prev) {
              URL.revokeObjectURL(prev);
            }

            this.thumbs[job.id] = URL.createObjectURL(blob);

            const index = this.queue.findIndex((item) => item.id === job.id);

            if (index !== -1) {
              this.queue.splice(index, 1);
            }
          } catch {
            // Preview missing or not ready yet. Only retry when we know one
            // should appear (fresh upload). Existing files get a single attempt.
            if (!job.retry || job.attempts + 1 >= PREVIEW_MAX_ATTEMPTS) {
              const index = this.queue.findIndex((item) => item.id === job.id);

              if (index !== -1) {
                this.queue.splice(index, 1);
              }

              return;
            }

            job.attempts += 1;
          }
        }),
      );
    } finally {
      this.polling = false;

      if (this.queue.length > 0) {
        // Jobs added while we were fetching (or remaining batch) must continue.
        void this.drainQueue();
      } else {
        this.clearPollInterval();
      }
    }
  }
}
