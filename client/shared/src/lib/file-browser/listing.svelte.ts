import { ValidationError } from "../api";
import { session } from "../session.svelte";
import { kv } from "../storage";
import { toastError } from "../toast";
import type { FileEntry, FileView } from "../types";

export type SortColumn = "kind" | "name" | "size" | "created";
export type SortDirection = "asc" | "desc";

export type SortState = {
  column: SortColumn;
  direction: SortDirection;
};

export const DEFAULT_SORT: SortState = {
  column: "kind",
  direction: "asc",
};

export type ListingCrumb = {
  id: string | null;
  name: string;
};

export type FileListingOptions = {
  rootId: () => string;
  locationId: () => string | null;
  /** Shell resets selection / drafts / share / preview / scroll. */
  onOpen: () => void;
  /** After a successful list (e.g. preview enqueue). */
  onLocationChange: (entries: FileView[]) => void;
};

/**
 * Directory listing, sort, ancestors, and entry mutations
 * for one FileBrowser mount.
 */
export class FileListing {
  /**
   * All entries either directly in a root or in a directory.
   */
  entries = $state<FileView[]>([]);
  loading = $state(true);
  latestLoad = 0;

  ancestors = $state<ListingCrumb[]>([]);
  /** Current directory entry when browsing inside a folder; null at root. */
  locationEntry = $state<FileEntry | null>(null);

  sort = $state<SortState>(readStoredSort());

  occurences = $derived.by(() => {
    const seen: Record<string, number> = {};
    const labels: Record<string, string> = {};

    for (const entry of this.entries) {
      const prior = seen[entry.file.name] ?? 0;

      seen[entry.file.name] = prior + 1;

      if (prior > 0) {
        labels[entry.file.id] = `${entry.file.name} (${prior})`;
      } else {
        labels[entry.file.id] = entry.file.name;
      }
    }

    return labels;
  });

  opts: FileListingOptions;

  constructor(opts: FileListingOptions) {
    this.opts = opts;
  }

  label(entry: FileView) {
    return this.occurences[entry.file.id] ?? entry.file.name;
  }

  asView(file: FileEntry): FileView {
    return { file, metadata: null };
  }

  onSortCycle() {
    kv.filesSort.set(this.sort);
    this.order();
  }

  order() {
    this.entries.sort((left, right) => {
      const a = left.file;
      const b = right.file;

      let byColumn = 0;

      switch (this.sort.column) {
        case "kind":
          byColumn = a.kind.localeCompare(b.kind);
          break;
        case "name":
          byColumn = a.name.localeCompare(b.name, undefined, {
            numeric: true,
            sensitivity: "base",
          });
          break;
        case "size":
          byColumn = a.sizeBytes - b.sizeBytes;
          break;
        case "created":
          byColumn = a.createdAt.localeCompare(b.createdAt);
          break;
      }

      const directed = this.sort.direction === "asc" ? byColumn : -byColumn;

      if (directed !== 0) {
        return directed;
      }

      const byName = a.name.localeCompare(b.name, undefined, {
        numeric: true,
        sensitivity: "base",
      });

      if (byName !== 0) {
        return byName;
      }

      if (a.createdAt !== b.createdAt) {
        return a.createdAt < b.createdAt ? -1 : 1;
      }

      if (a.id === b.id) {
        return 0;
      }

      return a.id < b.id ? -1 : 1;
    });
  }

  /** Drop the returned entry into the open folder. */
  insertEntry(created: FileEntry) {
    if (created.parent !== this.opts.locationId()) {
      return;
    }

    this.entries.push(this.asView(created));
    this.order();
  }

  remove(ids: Iterable<string>) {
    const gone = new Set(ids);
    this.entries = this.entries.filter((entry) => !gone.has(entry.file.id));
  }

  patchEntry(id: string, patch: (file: FileEntry) => void) {
    const index = this.entries.findIndex((item) => item.file.id === id);

    if (index === -1) {
      return;
    }

    patch(this.entries[index].file);
  }

  setEntrySize(id: string, sizeBytes: number) {
    this.patchEntry(id, (file) => {
      file.sizeBytes = sizeBytes;
    });
  }

  /**
   * Load ancestors of the current folder being viewed
   * if users land directly on a directory page.
   */
  async loadAncestors(
    rootId: string,
    parent: string | null,
    currentLoad: number,
  ) {
    if (parent === null) {
      this.ancestors = [];
      this.locationEntry = null;
      return;
    }

    // If the last ancestor is the current parent (dir), we're there
    // (but only if the trail has unique ids). Still refresh locationEntry
    // when the cached entry does not match the open folder.
    const tipMatches = (this.ancestors.at(-1)?.id ?? null) === parent;

    const uniqueIds =
      new Set(this.ancestors.map((a) => a.id)).size === this.ancestors.length;

    if (tipMatches && uniqueIds && this.locationEntry?.id === parent) {
      return;
    }

    try {
      const chain = await session.client.ancestors(rootId, parent);

      if (currentLoad !== this.latestLoad) {
        return;
      }

      this.ancestors = chain.map((entry) => ({
        id: entry.id,
        name: entry.name,
      }));
      this.locationEntry = chain.at(-1) ?? null;
    } catch (cause) {
      if (currentLoad !== this.latestLoad) {
        return;
      }

      this.ancestors = [];
      this.locationEntry = null;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not open this folder",
      );
    }
  }

  async open(rootId: string, parent: string | null) {
    const currentLoad = ++this.latestLoad;

    this.opts.onOpen();

    this.loading = true;
    this.entries = [];

    if (parent === null) {
      this.locationEntry = null;
    } else if (this.locationEntry?.id !== parent) {
      // Keep a matching cached entry (e.g. from openDirectory); clear stale ones.
      this.locationEntry = null;
    }

    this.loadAncestors(rootId, parent, currentLoad);

    try {
      const next = await session.client.list(rootId, parent);

      if (currentLoad !== this.latestLoad) {
        return;
      }

      this.entries = next.filter((entry) => entry.file.parent === parent);

      this.order();

      this.opts.onLocationChange(this.entries);
    } catch (cause) {
      if (currentLoad !== this.latestLoad) {
        return;
      }
      this.entries = [];
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not list this folder",
      );
    } finally {
      if (currentLoad === this.latestLoad) {
        this.loading = false;
      }
    }
  }

  openDirectory(
    entry: FileView,
    routeParentId: string | null,
    showDirectory: (parent: string | null) => void,
  ) {
    const id = entry.file.id;

    if (routeParentId === id) {
      return;
    }

    if (this.ancestors.at(-1)?.id !== id) {
      this.ancestors = this.ancestors.concat({
        id,
        name: entry.file.name,
      });
    }

    this.locationEntry = entry.file;
    showDirectory(id);
  }

  openParentDirectory(
    trailLength: number,
    parentId: string | null,
    showDirectory: (parent: string | null) => void,
  ) {
    if (trailLength < 2) {
      return;
    }

    const index = trailLength - 2;
    this.ancestors = this.ancestors.slice(0, index);
    showDirectory(parentId);
  }

  sliceAncestors(index: number) {
    this.ancestors = this.ancestors.slice(0, index);
  }

  async renameEntry(entry: FileView, name: string, canWrite: boolean) {
    if (!canWrite) {
      return;
    }

    try {
      await session.client.rename(this.opts.rootId(), entry.file.id, name);

      this.patchEntry(entry.file.id, (file) => {
        file.name = name;
      });
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ValidationError
            ? cause.field("name")?.message || "That name is not allowed"
            : `Could not rename ${this.label(entry)}`,
      );
      throw cause;
    }
  }

  applyShare(
    id: string,
    sharingMode: FileEntry["sharingMode"],
    shareExpiresAt: string | null,
  ) {
    this.patchEntry(id, (file) => {
      file.sharingMode = sharingMode;
      file.shareExpiresAt = shareExpiresAt;
    });

    if (this.locationEntry?.id === id) {
      this.locationEntry.sharingMode = sharingMode;
      this.locationEntry.shareExpiresAt = shareExpiresAt;
    }
  }

  /** Cascade share settings onto every visible child after sharing the open folder. */
  setSharingModeAll(
    sharingMode: FileEntry["sharingMode"],
    shareExpiresAt: string | null,
  ) {
    for (const item of this.entries) {
      item.file.sharingMode = sharingMode;
      item.file.shareExpiresAt = shareExpiresAt;
    }
  }
}

function readStoredSort(): SortState {
  const stored = kv.filesSort.get();
  return {
    column: stored.column as SortColumn,
    direction: stored.direction,
  };
}
