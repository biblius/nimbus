import { tick } from "svelte";
import { ApiError, ValidationError } from "../api";
import { session } from "../session.svelte";
import type { FileEntry } from "../types";

export type FileDraftsOptions = {
  rootId: () => string;
  locationId: () => string | null;
  getCanWrite: () => boolean;
  getScroller: () => HTMLElement | undefined;
  onCreated: (entry: FileEntry) => void;
};

/**
 * Inline "new folder" / "new text file" draft row for one FileBrowser mount.
 */
export class FileDrafts {
  draftKind = $state<"directory" | "text" | null>(null);
  drafting = $derived(this.draftKind !== null);

  directoryName = $state("");
  directoryError = $state("");
  directorySaving = $state(false);

  nameInput: HTMLInputElement | undefined;

  opts: FileDraftsOptions;

  constructor(opts: FileDraftsOptions) {
    this.opts = opts;
  }

  reset() {
    this.draftKind = null;
    this.directorySaving = false;
    this.directoryName = "";
    this.directoryError = "";
  }

  /** Select the editable stem; keep `.txt` unselected for text drafts. */
  selectDraftName(input: HTMLInputElement) {
    input.focus();

    if (this.draftKind === "text") {
      const dot = input.value.lastIndexOf(".");

      if (dot > 0) {
        input.setSelectionRange(0, dot);
        return;
      }
    }

    input.select();
  }

  focusDraft = (node: HTMLInputElement) => {
    this.nameInput = node;

    queueMicrotask(() => {
      this.selectDraftName(node);
    });

    return () => {
      if (this.nameInput === node) {
        this.nameInput = undefined;
      }
    };
  };

  async beginDraft(kind: "directory" | "text") {
    if (!this.opts.getCanWrite()) {
      return;
    }

    if (this.drafting) {
      if (this.nameInput) {
        this.selectDraftName(this.nameInput);
      }
      return;
    }

    this.directoryName = kind === "text" ? "untitled.txt" : "New folder";
    this.directoryError = "";
    this.directorySaving = false;

    this.draftKind = kind;

    await tick();

    this.opts.getScroller()?.scrollTo({ top: 0 });
  }

  cancelDirectory() {
    if (this.directorySaving) {
      return;
    }
    this.reset();
  }

  onDraftKey(event: KeyboardEvent) {
    if (event.isComposing) {
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      this.cancelDirectory();
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      void this.commitDirectory();
    }
  }

  directoryFailure(cause: unknown) {
    if (cause instanceof ValidationError) {
      return cause.field("name")?.message || "That name is not allowed";
    }
    if (cause instanceof TypeError) {
      return "The server could not be reached";
    }
    const noun = this.draftKind === "text" ? "file" : "folder";
    if (cause instanceof ApiError && cause.status === 403) {
      return `You cannot create a ${noun} here`;
    }
    if (cause instanceof ApiError && cause.status === 422) {
      return `That ${noun} name is not allowed`;
    }
    return `Could not create the ${noun}`;
  }

  /** Append `.txt` when the name has no extension; leave user-provided ones alone. */
  ensureTextExtension(name: string) {
    const dot = name.lastIndexOf(".");

    if (dot > 0 && dot < name.length - 1) {
      return name;
    }

    return `${name.replace(/\.+$/, "")}.txt`;
  }

  async commitDirectory() {
    if (!this.draftKind || this.directorySaving) {
      return;
    }

    const kind = this.draftKind;
    let name = this.directoryName.trim();

    if (!name) {
      this.directoryError =
        kind === "text"
          ? "File name cannot be empty"
          : "Directory name cannot be empty";
      this.nameInput?.focus();
      return;
    }

    if (kind === "text") {
      name = this.ensureTextExtension(name);
      this.directoryName = name;
    }

    const rootId = this.opts.rootId();
    const parent = this.opts.locationId();

    this.directorySaving = true;
    this.directoryError = "";

    try {
      const created =
        kind === "text"
          ? await session.client.createTextFile(rootId, parent, name)
          : await session.client.createDirectory(rootId, parent, name);

      // Prevent inserting the entry if the view has changed
      if (
        this.opts.rootId() !== rootId ||
        this.opts.locationId() !== parent
      ) {
        return;
      }

      this.opts.onCreated(created);

      this.draftKind = null;
      this.directoryName = "";
      this.directoryError = "";
    } catch (cause) {
      if (
        this.opts.rootId() !== rootId ||
        this.opts.locationId() !== parent
      ) {
        return;
      }
      this.directoryError = this.directoryFailure(cause);
      this.nameInput?.focus();
    } finally {
      if (
        this.opts.rootId() === rootId &&
        this.opts.locationId() === parent
      ) {
        this.directorySaving = false;
      }
    }
  }
}
