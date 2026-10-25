import { publicShareLink } from "../server-url";
import { session } from "../session.svelte";
import { toastError, toastSuccess } from "../toast";
import type { FileEntry, FileView, SharingMode, ShareFile } from "../types";

export type FileSharingOptions = {
  rootId: () => string;
  getCanShare: () => boolean;
  getLocationEntry: () => FileEntry | null;
  applyShare: (
    id: string,
    sharingMode: SharingMode | null,
    shareExpiresAt: string | null,
  ) => void;
  setSharingModeAll: (
    sharingMode: SharingMode | null,
    shareExpiresAt: string | null,
  ) => void;
  onShared: () => void;
};

/**
 * Public share dialog + link helpers for one FileBrowser mount.
 */
export class FileSharing {
  opts: FileSharingOptions;

  /** Entry being shared, or null when sharing the open directory. */
  shareTarget = $state<FileEntry | null>(null);
  shareOpen = $state(false);
  shareBusy = $state(false);

  /** Extra IDs when sharing a multi-selection (includes `shareTarget.id`). */
  shareIds = $state<string[]>([]);

  // `$derived.by` (lazy): field inits run before the constructor sets `opts`.
  shareSubject = $derived.by(
    () =>
      this.shareTarget ??
      (this.shareOpen ? this.opts.getLocationEntry() : null),
  );

  constructor(opts: FileSharingOptions) {
    this.opts = opts;
  }

  /** Cleared when the shell opens a new directory listing. */
  resetOnOpen() {
    this.shareOpen = false;
    this.shareBusy = false;
    this.shareTarget = null;
    this.shareIds = [];
  }

  publicLink(entry: FileView) {
    return publicShareLink(entry.file.id);
  }

  /**
   * Open the file sharing dialog for the entry.
   * Actions taken in the dialog apply to all items in `ids`.
   */
  openShareDialog(entry: FileEntry, ids: string[] = [entry.id]) {
    if (!this.opts.getCanShare()) {
      return;
    }

    this.shareTarget = entry;
    this.shareIds = ids.slice();
    this.shareOpen = true;
  }

  openDirectoryShareDialog() {
    const locationEntry = this.opts.getLocationEntry();

    if (!locationEntry || !this.opts.getCanShare()) {
      return;
    }

    this.shareTarget = null;
    this.shareIds = [locationEntry.id];
    this.shareOpen = true;
  }

  closeShareDialog() {
    if (this.shareBusy) {
      return;
    }

    this.shareOpen = false;
    this.shareTarget = null;
    this.shareIds = [];
  }

  async copyLink(entry: FileView, message = "Public link copied") {
    try {
      await navigator.clipboard.writeText(this.publicLink(entry));
      toastSuccess(message);
    } catch {
      toastError(`Could not copy link for ${entry.file.name}`);
    }
  }

  async confirmShare(input: ShareFile) {
    if (!this.shareSubject || this.shareBusy) {
      return;
    }

    const entry = this.shareSubject;
    const ids = this.shareIds.length > 0 ? this.shareIds.slice() : [entry.id];
    const rootId = this.opts.rootId();
    const expiresAt = input.shareExpiresAt ?? null;
    const cascading = this.shareTarget == null;

    this.shareBusy = true;

    try {
      await session.client.shareFiles(rootId, ids, {
        sharingMode: input.sharingMode,
        shareExpiresAt: expiresAt,
      });

      if (this.opts.rootId() === rootId) {
        for (const id of ids) {
          this.opts.applyShare(id, input.sharingMode, expiresAt);
        }
        if (cascading) {
          this.opts.setSharingModeAll(input.sharingMode, expiresAt);
        }
      }

      if (ids.length === 1) {
        await this.copyLink(
          { file: entry, metadata: null },
          `Sharing ${entry.name}`,
        );
      } else {
        toastSuccess(`Sharing ${ids.length} items`);
      }

      this.shareOpen = false;
      this.shareTarget = null;
      this.shareIds = [];
      this.opts.onShared();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : ids.length === 1
            ? `Could not share ${entry.name}`
            : `Could not share ${ids.length} items`,
      );
    } finally {
      if (this.opts.rootId() === rootId) {
        this.shareBusy = false;
      }
    }
  }

  async confirmUnshare() {
    if (!this.shareSubject || this.shareBusy) {
      return;
    }

    const entry = this.shareSubject;
    const ids = this.shareIds.length > 0 ? this.shareIds.slice() : [entry.id];
    const rootId = this.opts.rootId();
    const cascading = this.shareTarget == null;

    this.shareBusy = true;

    try {
      await session.client.unshareFiles(rootId, ids);

      if (this.opts.rootId() === rootId) {
        for (const id of ids) {
          this.opts.applyShare(id, null, null);
        }
        if (cascading) {
          this.opts.setSharingModeAll(null, null);
        }
      }

      toastSuccess(
        ids.length === 1
          ? `Stopped sharing ${entry.name}`
          : `Stopped sharing ${ids.length} items`,
      );
      this.shareOpen = false;
      this.shareTarget = null;
      this.shareIds = [];
      this.opts.onShared();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : ids.length === 1
            ? `Could not stop sharing ${entry.name}`
            : `Could not stop sharing ${ids.length} items`,
      );
    } finally {
      if (this.opts.rootId() === rootId) {
        this.shareBusy = false;
      }
    }
  }
}
