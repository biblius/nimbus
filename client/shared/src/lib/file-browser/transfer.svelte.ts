import type { DestinationRequest } from "./destination";
import { PARENT_DROPPABLE_ID } from "./drop";
import { session } from "../session.svelte";
import { toastError } from "../toast";
import type { FileView, Root } from "../types";

export type FileTransferOptions = {
  root: () => Root;
  locationId: () => string | null;
  getCanWrite: () => boolean;
  getWritableRoots: () => Root[];
  getEntries: () => FileView[];
  getSelected: () => string[];
  label: (entry: FileView) => string;
  removeMovedEntries: (ids: string[]) => void;
  exitSelectMode: () => void;
  getShowParentDrop: () => boolean;
  getTrailLength: () => number;
  getUpParent: () => string | null;
};

/**
 * Copy / move destination picker and drag-drop move for one FileBrowser mount.
 */
export class FileTransfer {
  destination = $state<DestinationRequest | null>(null);

  opts: FileTransferOptions;

  constructor(opts: FileTransferOptions) {
    this.opts = opts;
  }

  /**
   * IDs acted on by drag or context-menu actions.
   * If the source is already selected, the whole selection is used.
   */
  actionIds(sourceId: string): string[] {
    const selected = this.opts.getSelected();
    return selected.includes(sourceId) ? selected.slice() : [sourceId];
  }

  dragIds(sourceId: string): string[] {
    return this.actionIds(sourceId);
  }

  dragPreview(sourceId: string): {
    entry: FileView | undefined;
    name: string;
    count: number;
  } {
    const ids = this.dragIds(sourceId);
    const entry = this.opts
      .getEntries()
      .find((item) => item.file.id === sourceId);

    return {
      entry,
      name: entry ? this.opts.label(entry) : "Item",
      count: ids.length,
    };
  }

  startCopy(ids: string[]) {
    if (ids.length === 0) {
      return;
    }

    const destinations = this.opts.getWritableRoots();

    if (destinations.length === 0) {
      return;
    }

    const entries = this.opts.getEntries();
    const single =
      ids.length === 1
        ? entries.find((item) => item.file.id === ids[0])
        : undefined;

    const title = single
      ? `Copy ${this.opts.label(single)}`
      : `Copy ${ids.length} items`;

    const description = single
      ? `Pick a destination for ${this.opts.label(single)} copy.`
      : `Pick a destination for ${ids.length} items.`;

    const sourceRootId = this.opts.root().id;

    this.destination = {
      roots: destinations,
      title,
      description,
      confirmLabel: "Copy",
      onConfirm: async (targetRoot: Root, parent: string | null) => {
        try {
          await session.client.copy(sourceRootId, ids, targetRoot.id, parent);
          this.destination = null;
        } catch (cause) {
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : single
                ? `Could not copy ${this.opts.label(single)}`
                : `Could not copy ${ids.length} items`,
          );
        }
      },
    };
  }

  startMove(ids: string[]) {
    if (ids.length === 0 || !this.opts.getCanWrite()) {
      return;
    }

    const entries = this.opts.getEntries();
    const single =
      ids.length === 1
        ? entries.find((item) => item.file.id === ids[0])
        : undefined;

    const title = single
      ? `Move ${this.opts.label(single)}`
      : `Move ${ids.length} items`;

    const description = single
      ? `Pick a destination for ${this.opts.label(single)}.`
      : `Pick a destination for ${ids.length} items.`;

    this.destination = {
      roots: [this.opts.root()],
      title,
      description,
      confirmLabel: "Move",
      onConfirm: async (_targetRoot: Root, parent: string | null) => {
        const rootId = this.opts.root().id;
        const currentParent = this.opts.locationId();

        try {
          await session.client.move(rootId, ids, parent);

          if (
            this.opts.root().id !== rootId ||
            this.opts.locationId() !== currentParent
          ) {
            this.destination = null;
            return;
          }

          // Filter out all items now gone from this directory
          if (parent !== currentParent) {
            this.opts.removeMovedEntries(ids);
          }

          this.destination = null;
          this.opts.exitSelectMode();
        } catch (cause) {
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : single
                ? `Could not move ${this.opts.label(single)}`
                : `Could not move ${ids.length} items`,
          );
        }
      },
    };
  }

  async onDragEnd(event: {
    canceled: boolean;
    operation: {
      source: { id: unknown } | null;
      target: { id: unknown } | null;
    };
  }) {
    if (event.canceled || !this.opts.getCanWrite()) {
      return;
    }

    const sourceId = event.operation.source?.id;
    const targetId = event.operation.target?.id;

    if (typeof sourceId !== "string" || typeof targetId !== "string") {
      return;
    }

    if (sourceId === targetId) {
      return;
    }

    let newParent: string | null;

    if (targetId === PARENT_DROPPABLE_ID) {
      if (!this.opts.getShowParentDrop() || this.opts.getTrailLength() < 2) {
        return;
      }

      newParent = this.opts.getUpParent();
    } else {
      const target = this.opts
        .getEntries()
        .find((entry) => entry.file.id === targetId);

      if (!target || target.file.kind !== "directory") {
        return;
      }

      newParent = targetId;
    }

    const ids = this.dragIds(sourceId).filter((id) => id !== targetId);

    if (ids.length === 0) {
      return;
    }

    const rootId = this.opts.root().id;
    const parent = this.opts.locationId();

    try {
      await session.client.move(rootId, ids, newParent);

      if (this.opts.root().id !== rootId || this.opts.locationId() !== parent) {
        return;
      }

      this.opts.removeMovedEntries(ids);
    } catch (cause) {
      const entries = this.opts.getEntries();
      const single =
        ids.length === 1
          ? entries.find((item) => item.file.id === ids[0])
          : undefined;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : single
            ? `Could not move ${this.opts.label(single)}`
            : `Could not move ${ids.length} items`,
      );
    }
  }
}
