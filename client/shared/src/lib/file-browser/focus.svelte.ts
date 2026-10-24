import { tick, untrack } from "svelte";
import { navigate } from "../router";
import { session } from "../session.svelte";
import { toastError } from "../toast";
import type { FileView } from "../types";

export type FileFocusScrollLayout = {
  columns: number;
  gap: number;
  itemHeight: number;
};

export type FileFocusOptions = {
  getScroller: () => HTMLElement | undefined;
  getVirtualLayout: () => FileFocusScrollLayout;
  getShowParentDrop: () => boolean;
  onSelect: (ids: string[]) => void;
};

/**
 * Deep-link focus (`?file=ID`): select + scroll, or redirect to the file's parent.
 * Shell keeps a thin `$effect` that reads route/list state and calls `apply`.
 */
export class FileFocus {
  /** Last `root/parent/file` focus we already applied (avoids re-scroll on list churn). */
  focusedFileKey: string | null = null;

  opts: FileFocusOptions;

  constructor(opts: FileFocusOptions) {
    this.opts = opts;
  }

  scrollToEntry(index: number) {
    const el = this.opts.getScroller();

    if (!el || index < 0) {
      return;
    }

    const { columns, gap, itemHeight } = this.opts.getVirtualLayout();
    const cols = Math.max(1, columns);
    const leading = this.opts.getShowParentDrop() ? 1 : 0;
    const row = Math.floor((index + leading) / cols);
    const top = row * (itemHeight + gap);
    const target = Math.max(0, top - el.clientHeight / 3);

    el.scrollTo({ top: target, behavior: "smooth" });
  }

  /**
   * Apply focus for `?file=ID` on a directory route.
   * Returns a teardown that cancels an in-flight ancestors lookup.
   */
  apply(args: {
    fileId: string | null;
    rootId: string;
    parentId: string | null;
    list: FileView[];
    isLoading: boolean;
  }): (() => void) | undefined {
    const { fileId, rootId, parentId, list, isLoading } = args;

    if (!fileId) {
      this.focusedFileKey = null;
      return;
    }

    if (isLoading) {
      return;
    }

    const focusKey = `${rootId}\0${parentId ?? ""}\0${fileId}`;

    if (this.focusedFileKey === focusKey) {
      return;
    }

    const index = list.findIndex((entry) => entry.file.id === fileId);

    if (index >= 0) {
      this.focusedFileKey = focusKey;
      this.opts.onSelect([fileId]);

      untrack(() => {
        void tick().then(() => this.scrollToEntry(index));
      });
      return;
    }

    let cancelled = false;

    untrack(() => {
      void session.client
        .ancestors(rootId, fileId)
        .then((chain) => {
          if (cancelled) {
            return;
          }

          const entry = chain.at(-1);

          if (!entry) {
            this.focusedFileKey = focusKey;
            return;
          }

          const entryParent = entry.parent ?? null;

          if (entryParent === (parentId ?? null)) {
            // Expected in this listing but missing (deleted / filtered out).
            this.focusedFileKey = focusKey;
            return;
          }

          const search = { file: fileId };

          if (entryParent) {
            navigate("/files/:root_id/dir/:parent_id", {
              params: { root_id: rootId, parent_id: entryParent },
              search,
              replace: true,
            });
            return;
          }

          navigate("/files/:root_id", {
            params: { root_id: rootId },
            search,
            replace: true,
          });
        })
        .catch((cause) => {
          if (cancelled) {
            return;
          }

          this.focusedFileKey = focusKey;
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : "Could not open that file",
          );
        });
    });

    return () => {
      cancelled = true;
    };
  }
}
