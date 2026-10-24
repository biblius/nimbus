import { hasFinePointerHover } from "../utils";

export type FileSelectionOptions = {
  /** Ordered entry ids for the current listing (Shift+click range). */
  getOrderedIds: () => string[];
};

/**
 * Multi-select / select-mode state for one FileBrowser mount.
 * Does not own delete, copy, or move.
 */
export class FileSelection {
  /**
   * Empty on component load, but after a user clicks a list item
   * always has at least one entry until they have deleted/moved it
   * in which case it becomes empty again.
   * When this has length >= 2 the multiselect toolbar appears, and
   * right-click / action menus on a selected item apply to all of them.
   */
  selected = $state<string[]>([]);

  /**
   * Anchor for Shift+click range selection
   * (last plain / Ctrl click).
   */
  selectionAnchor = $state<string | null>(null);

  /**
   * When true, plain click/tap toggles.
   * Entered via long-press (touch) or Ctrl/Shift selection (desktop).
   */
  selectMode = $state(false);

  opts: FileSelectionOptions;

  constructor(opts: FileSelectionOptions) {
    this.opts = opts;
  }

  toggle(id: string) {
    const index = this.selected.indexOf(id);

    if (index === -1) {
      this.selected.push(id);
      return;
    }

    this.selected.splice(index, 1);
  }

  clearSelection() {
    this.selected = [];
    this.selectionAnchor = null;
    this.selectMode = false;
  }

  remove(ids: Iterable<string>) {
    const gone = new Set(ids);
    this.selected = this.selected.filter((id) => !gone.has(id));
  }

  /**
   * Selection:
   * - Click: select only this item (unless selectMode, then toggle)
   * - Fine pointer + click outside selection (no Ctrl/Shift): select only that item, exit selectMode
   * - Ctrl/Cmd+Click: toggle and enter select mode while anything remains selected
   * - Shift+Click: contiguous range from anchor (desktop); enters select mode
   * - Long-press: enter select mode with only this item selected
   */
  selectEntry(id: string, event: MouseEvent) {
    const additive = event.ctrlKey || event.metaKey;
    const range = event.shiftKey;
    const ids = this.opts.getOrderedIds();

    // User has pressed Shift + Click
    if (range && this.selected.length > 0) {
      let anchorId = this.selected[0];

      if (
        this.selectionAnchor != null &&
        ids.includes(this.selectionAnchor)
      ) {
        anchorId = this.selectionAnchor;
      }

      const from = ids.indexOf(anchorId);
      const to = ids.indexOf(id);

      if (from !== -1 && to !== -1) {
        const start = Math.min(from, to);
        const end = Math.max(from, to);
        this.selected = ids.slice(start, end + 1);
        this.selectMode = true;
        return;
      }
    }

    // Mouse: plain click outside the current selection leaves select mode.
    if (
      this.selectMode &&
      !additive &&
      !range &&
      hasFinePointerHover() &&
      !this.selected.includes(id)
    ) {
      this.selected = [id];
      this.selectionAnchor = id;
      this.selectMode = false;
      return;
    }

    if (additive || this.selectMode) {
      this.toggle(id);
      this.selectionAnchor = id;
      this.selectMode = this.selected.length > 0;
      return;
    }

    this.selected = [id];
    this.selectionAnchor = id;
  }

  /** Long-press: enter select mode with this item as the only selection. */
  longSelectEntry(id: string) {
    this.selectMode = true;
    this.selected = [id];
    this.selectionAnchor = id;
  }

}
