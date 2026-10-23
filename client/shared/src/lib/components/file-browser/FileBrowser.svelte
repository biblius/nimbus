<script lang="ts">
  import {
    Check,
    ChevronDown,
    Copy,
    Download,
    File as FileIcon,
    Folder,
    FolderInput,
    Link2,
    Plus,
    Search,
    Share2,
    Trash,
    Upload,
    X,
  } from "@lucide/svelte";
  import { PointerActivationConstraints, PointerSensor } from "@dnd-kit/dom";
  import { DragDropProvider, DragOverlay } from "@dnd-kit/svelte";
  import { onDestroy, tick, untrack } from "svelte";
  import { navigate } from "../../router";
  import { ApiError, ValidationError } from "../../api";
  import { PreviewQueue } from "../../previewQueue.svelte";
  import { session } from "../../session.svelte";
  import { toastError, toastSuccess } from "../../toast";
  import {
    hasShareAccess,
    hasWriteAccess,
    type FileEntry,
    type FileView,
    type Root,
    type ShareFile,
    type SharingMode,
  } from "../../types";
  import { kv, type FilesViewMode } from "../../storage";
  import { UploadHandler } from "../../uploadHandler.svelte";
  import {
    FILE_ICON_CHIP_CLASS,
    fileIconChipStyle,
    isPreviewable,
  } from "../../fileKind";
  import { hasFinePointerHover } from "../../utils";
  import DestinationPicker, {
    type DestinationRequest,
  } from "./DestinationPicker.svelte";
  import FileListItem from "./FileListItem.svelte";
  import FilePathCrumbs, { type Crumb } from "./FilePathCrumbs.svelte";
  import FileSearchResults from "./FileSearchResults.svelte";
  import FileShareOptions from "./FileShareOptions.svelte";
  import FileTileItem from "./FileTileItem.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import FileViewModeMenu from "./FileViewModeMenu.svelte";
  import ParentDropItem, { PARENT_DROPPABLE_ID } from "./ParentDropItem.svelte";
  import PendingFileUpload from "./PendingFileUpload.svelte";
  import RootAccessBadges from "./RootAccessBadges.svelte";
  import Button from "../ui/button/button.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import Input from "../ui/input/input.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import Menu from "../ui/menu/menu.svelte";
  import SortHeader from "../ui/sort-header/sort-header.svelte";
  import VirtualScroller from "../ui/virtual-scroller/VirtualScroller.svelte";

  let {
    folder: root,
    parentId = null,
    roots,
  }: {
    folder: Root;
    /** Directory open in this root. Null lists the root itself. */
    parentId?: string | null;
    roots: Root[];
  } = $props();

  const sensors = [
    PointerSensor.configure({
      activationConstraints: [
        new PointerActivationConstraints.Distance({ value: 8 }),
      ],
      preventActivation: (event) =>
        event.target instanceof Element &&
        Boolean(event.target.closest("[data-no-drag]")),
    }),
  ];

  type SortColumn = "kind" | "name" | "size" | "created";
  type SortDirection = "asc" | "desc";

  const DEFAULT_SORT: { column: SortColumn; direction: SortDirection } = {
    column: "kind",
    direction: "asc",
  };

  const columns =
    "grid grid-cols-[minmax(0,1fr)_7.5rem] items-center gap-x-3 px-3 @lg:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_7.5rem] @2xl:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_minmax(0,11.5rem)_7.5rem]";

  let entries = $state<FileView[]>([]);
  let searchQuery = $state("");
  const searching = $derived(searchQuery.trim().length > 0);

  /**
   * Empty on component load, but after a user clicks a list item
   * always has at least one entry until they have deleted/moved it
   * in which case it becomes empty again.
   * When this has length >= 2 the multiselect toolbar appears, and
   * right-click / action menus on a selected item apply to all of them.
   */
  let selected = $state<string[]>([]);

  /**
   * Anchor for Shift+click range selection
   * (last plain / Ctrl click).
   */
  let selectionAnchor = $state<string | null>(null);

  /**
   * When true, plain click/tap toggles.
   * Entered via long-press (touch) or Ctrl/Shift selection (desktop).
   */
  let selectMode = $state(false);

  /**
   * Controls deletion confirmation dialog.
   */
  let deleteOpen = $state(false);

  /**
   * True when the users has confirmed deletion
   * and the request is processing.
   */
  let deleting = $state(false);

  /**
   * List of file IDs the user has selected.
   * Gets set when they click on delete.
   * Sent to the server for trashing on confirming.
   */
  let pendingDelete = $state<string[]>([]);
  let previewFileId = $state<string | null>(null);
  let destination = $state<DestinationRequest | null>(null);
  let viewMode = $state<FilesViewMode>(kv.filesViewMode.get());

  const previewEntries = $derived(entries.filter(isPreviewable));

  const canWrite = $derived(hasWriteAccess(root));
  const canShare = $derived(hasShareAccess(root));

  const writableRoots = $derived(roots.filter(hasWriteAccess));

  const hasCopyDestination = $derived(writableRoots.length > 0);

  const writeDeniedTitle = "You do not have write access";
  const shareDeniedTitle = "You do not have share access";

  function copyUnavailableTitle(): string {
    if (hasCopyDestination) {
      return "Copy selected";
    }
    return "There are no writable folders to copy to";
  }

  function directoryShareTitle(): string {
    if (!canShare) {
      return shareDeniedTitle;
    }
    if (locationEntry?.sharingMode) {
      return "Shared";
    }
    return "Share folder";
  }

  function directoryShareAriaLabel(): string {
    if (!canShare) {
      return shareDeniedTitle;
    }
    if (locationEntry?.sharingMode) {
      return "Folder is shared - manage sharing";
    }
    return "Share this folder";
  }

  const tiles = $derived(
    viewMode === "tiles-small" || viewMode === "tiles-large",
  );

  const tileSize = $derived<"small" | "large">(
    viewMode === "tiles-large" ? "large" : "small",
  );

  const listDensity = $derived<"default" | "compact">(
    viewMode === "compact" ? "compact" : "default",
  );

  const storedSort = kv.filesSort.get();

  let sort = $state<{ column: SortColumn; direction: SortDirection }>({
    column: storedSort.column as SortColumn,
    direction: storedSort.direction,
  });

  const occurences = $derived.by(() => {
    const seen: Record<string, number> = {};
    const labels: Record<string, string> = {};

    for (const entry of entries) {
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

  let loading = $state(true);
  let latestLoad = 0;

  let draftKind = $state<"directory" | "text" | null>(null);
  const drafting = $derived(draftKind !== null);

  let directoryName = $state("");
  let directoryError = $state("");
  let directorySaving = $state(false);

  let uploadMenuOpen = $state(false);
  let newMenuOpen = $state(false);

  let ancestors = $state<Crumb[]>([]);

  /** Current directory entry when browsing inside a folder; null at root. */
  let locationEntry = $state<FileEntry | null>(null);

  /** Entry being shared, or null when sharing the open directory. */
  let shareTarget = $state<FileEntry | null>(null);
  /** Extra IDs when sharing a multi-selection (includes `shareTarget.id`). */
  let shareIds = $state<string[]>([]);
  let shareOpen = $state(false);
  let shareBusy = $state(false);

  let fileInput = $state<HTMLInputElement | undefined>();
  let folderInput = $state<HTMLInputElement | undefined>();

  let scroller = $state<HTMLElement | undefined>();
  let viewportWidth = $state(0);

  let nameInput: HTMLInputElement | undefined;

  const virtualLayout = $derived.by(() => {
    if (!tiles) {
      return {
        columns: 1,
        gap: 0,
        itemHeight: listDensity === "compact" ? 44 : 64,
      };
    }

    const large = tileSize === "large";

    const rem =
      typeof document !== "undefined"
        ? parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
        : 16;

    const minTrack = (large ? 9.5 : 6.25) * rem;

    const gap = large ? 8 : 4;

    const pad = large ? 16 : 12;

    const inner = Math.max(0, viewportWidth - pad);

    const columns = Math.max(1, Math.floor((inner + gap) / (minTrack + gap)));

    const colWidth =
      columns > 0 ? (inner - gap * (columns - 1)) / columns : minTrack;

    const itemHeight = colWidth + (large ? 52 : 40);

    return { columns, gap, itemHeight };
  });

  /**
   * Derived from ancestors. Always puts the root in front.
   */
  const trail = $derived.by(() => {
    const crumbs: Crumb[] = [{ id: null, name: root.name }];

    for (const crumb of ancestors) {
      crumbs.push(crumb);
    }

    return crumbs;
  });

  /**
   * Derived from the trail, represents the current location in the file browser,
   * i.e. which directory are we located in.
   */
  const location = $derived(trail[trail.length - 1]);

  const previews = new PreviewQueue({
    getRootId: () => root.id,
    getParentId: () => location.id,
    getSize: () => (tiles ? 256 : 64),
  });

  const uploads = new UploadHandler({
    getRootId: () => root.id,
    getParentId: () => location.id,
    onCreated: (entry) => {
      insertEntry(entry);
      previews.enqueue(entry, true);
    },
  });

  /** Parent of the current folder; null when moving into the root. */
  const upParent = $derived(
    trail.length >= 2 ? trail[trail.length - 2].id : null,
  );

  const showParentDrop = $derived(location.id !== null);

  const shareSubject = $derived(
    shareTarget ?? (shareOpen ? locationEntry : null),
  );

  const deleteDescription = $derived.by(() => {
    if (pendingDelete.length === 1) {
      const entry = entries.find((item) => item.file.id === pendingDelete[0]);
      return `Delete ${entry ? label(entry) : "this item"}?`;
    }
    return `Delete ${pendingDelete.length} items?`;
  });

  $effect(() => {
    // Deliberate assignment so $effect keeps track of these
    const rootId = root.id;
    const parent = parentId;

    // Untrack the open function, since it mutates state and would trigger
    // the effect indefinitely.
    untrack(() => {
      open(rootId, parent);
    });
  });

  $effect(() => {
    root.id;
    searchQuery = "";
  });

  $effect(() => {
    const next = tiles;

    if (seenTiles !== undefined && seenTiles !== next) {
      previews.refreshAll(entries.map((entry) => entry.file));
    }

    seenTiles = next;
  });

  $effect(() => {
    const node = scroller;

    if (!node) {
      viewportWidth = 0;
      return;
    }

    const ro = new ResizeObserver(() => {
      viewportWidth = node.clientWidth;
    });

    ro.observe(node);
    viewportWidth = node.clientWidth;

    return () => ro.disconnect();
  });

  /**
   * Load ancestors of the current folder being viewed
   * if users land directly on a directory page.
   */
  async function loadAncestors(
    rootId: string,
    parent: string | null,
    currentLoad: number,
  ) {
    if (parent === null) {
      ancestors = [];
      locationEntry = null;
      return;
    }

    // If the last ancestor is the current parent (dir), we're there
    // (but only if the trail has unique ids). Still refresh locationEntry
    // when the cached entry does not match the open folder.
    const tipMatches = (ancestors.at(-1)?.id ?? null) === parent;

    const uniqueIds =
      new Set(ancestors.map((a) => a.id)).size === ancestors.length;

    if (tipMatches && uniqueIds && locationEntry?.id === parent) {
      return;
    }

    try {
      const chain = await session.client.ancestors(rootId, parent);

      if (currentLoad !== latestLoad) {
        return;
      }

      ancestors = chain.map((entry) => ({
        id: entry.id,
        name: entry.name,
      }));
      locationEntry = chain.at(-1) ?? null;
    } catch (cause) {
      if (currentLoad !== latestLoad) {
        return;
      }

      ancestors = [];
      locationEntry = null;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not open this folder",
      );
    }
  }

  function showDirectory(parent: string | null) {
    if (parent) {
      navigate("/files/:root_id/dir/:parent_id", {
        params: { root_id: root.id, parent_id: parent },
      });
      return;
    }
    navigate("/files/:root_id", { params: { root_id: root.id } });
  }

  function resetDraft() {
    draftKind = null;
    directorySaving = false;
    directoryName = "";
    directoryError = "";
  }

  // Swap cached thumbs when switching between list (64) and tiles (256).
  let seenTiles: boolean | undefined;

  onDestroy(() => previews.clear());

  async function open(rootId: string, parent: string | null) {
    const currentLoad = ++latestLoad;

    resetDraft();

    loading = true;

    previews.clear();

    entries = [];
    selected = [];
    selectionAnchor = null;
    selectMode = false;
    previewFileId = null;
    shareOpen = false;
    shareBusy = false;
    shareTarget = null;
    shareIds = [];
    scroller?.scrollTo({ top: 0 });

    if (parent === null) {
      locationEntry = null;
    } else if (locationEntry?.id !== parent) {
      // Keep a matching cached entry (e.g. from openDirectory); clear stale ones.
      locationEntry = null;
    }

    loadAncestors(rootId, parent, currentLoad);

    try {
      const next = await session.client.list(rootId, parent);

      if (currentLoad !== latestLoad) {
        return;
      }

      entries = next.filter((entry) => entry.file.parent === parent);

      order();
    } catch (cause) {
      if (currentLoad !== latestLoad) {
        return;
      }
      entries = [];
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not list this folder",
      );
    } finally {
      if (currentLoad === latestLoad) {
        loading = false;
      }
    }
  }

  function onSortCycle() {
    kv.filesSort.set(sort);
    order();
  }

  function order() {
    entries.sort((left, right) => {
      const a = left.file;
      const b = right.file;

      let byColumn = 0;

      switch (sort.column) {
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

      const directed = sort.direction === "asc" ? byColumn : -byColumn;

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

  function asView(file: FileEntry): FileView {
    return { file, metadata: null };
  }

  /// Drop the returned entry into the open folder.
  function insertEntry(created: FileEntry) {
    if (created.parent !== location.id) {
      return;
    }

    entries.push(asView(created));
    order();
  }

  function label(entry: FileView) {
    return occurences[entry.file.id] ?? entry.file.name;
  }

  function openDirectory(entry: FileView) {
    const id = entry.file.id;
    if (parentId === id) {
      return;
    }
    if (ancestors.at(-1)?.id !== id) {
      ancestors = ancestors.concat({
        id,
        name: entry.file.name,
      });
    }
    locationEntry = entry.file;
    showDirectory(id);
  }

  function openParentDirectory() {
    if (trail.length < 2) {
      return;
    }

    const index = trail.length - 2;
    const parent = trail[index].id;

    ancestors = ancestors.slice(0, index);
    showDirectory(parent);
  }

  /** Select the editable stem; keep `.txt` unselected for text drafts. */
  function selectDraftName(input: HTMLInputElement) {
    input.focus();

    if (draftKind === "text") {
      const dot = input.value.lastIndexOf(".");

      if (dot > 0) {
        input.setSelectionRange(0, dot);
        return;
      }
    }

    input.select();
  }

  function focusDraft(node: HTMLInputElement) {
    nameInput = node;

    queueMicrotask(() => {
      selectDraftName(node);
    });

    return () => {
      if (nameInput === node) {
        nameInput = undefined;
      }
    };
  }

  async function beginDraft(kind: "directory" | "text") {
    if (!canWrite) {
      return;
    }

    if (drafting) {
      if (nameInput) {
        selectDraftName(nameInput);
      }
      return;
    }

    directoryName = kind === "text" ? "untitled.txt" : "New folder";
    directoryError = "";
    directorySaving = false;

    draftKind = kind;

    await tick();

    scroller?.scrollTo({ top: 0 });
  }

  function cancelDirectory() {
    if (directorySaving) {
      return;
    }
    resetDraft();
  }

  function onDraftKey(event: KeyboardEvent) {
    if (event.isComposing) {
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancelDirectory();
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      commitDirectory();
    }
  }

  function directoryFailure(cause: unknown) {
    if (cause instanceof ValidationError) {
      return cause.field("name")?.message || "That name is not allowed";
    }
    if (cause instanceof TypeError) {
      return "The server could not be reached";
    }
    const noun = draftKind === "text" ? "file" : "folder";
    if (cause instanceof ApiError && cause.status === 403) {
      return `You cannot create a ${noun} here`;
    }
    if (cause instanceof ApiError && cause.status === 422) {
      return `That ${noun} name is not allowed`;
    }
    return `Could not create the ${noun}`;
  }

  /** Append `.txt` when the name has no extension; leave user-provided ones alone. */
  function ensureTextExtension(name: string) {
    const dot = name.lastIndexOf(".");

    if (dot > 0 && dot < name.length - 1) {
      return name;
    }

    return `${name.replace(/\.+$/, "")}.txt`;
  }

  async function commitDirectory() {
    if (!draftKind || directorySaving) {
      return;
    }

    const kind = draftKind;
    let name = directoryName.trim();

    if (!name) {
      directoryError =
        kind === "text"
          ? "File name cannot be empty"
          : "Directory name cannot be empty";
      nameInput?.focus();
      return;
    }

    if (kind === "text") {
      name = ensureTextExtension(name);
      directoryName = name;
    }

    const rootId = root.id;
    const parent = location.id;

    directorySaving = true;
    directoryError = "";

    try {
      const created =
        kind === "text"
          ? await session.client.createTextFile(rootId, parent, name)
          : await session.client.createDirectory(rootId, parent, name);

      // Prevent inserting the entry if the view has changed
      if (root.id !== rootId || location.id !== parent) {
        return;
      }

      insertEntry(created);

      draftKind = null;
      directoryName = "";
      directoryError = "";
    } catch (cause) {
      if (root.id !== rootId || location.id !== parent) {
        return;
      }
      directoryError = directoryFailure(cause);
      nameInput?.focus();
    } finally {
      if (root.id === rootId && location.id === parent) {
        directorySaving = false;
      }
    }
  }

  function toggle(id: string) {
    const index = selected.indexOf(id);

    if (index === -1) {
      selected.push(id);
      return;
    }

    selected.splice(index, 1);
  }

  function clearSelection() {
    selected = [];
    selectionAnchor = null;
    selectMode = false;
  }

  /**
   * Selection:
   * - Click: select only this item (unless selectMode, then toggle)
   * - Fine pointer + click outside selection (no Ctrl/Shift): select only that item, exit selectMode
   * - Ctrl/Cmd+Click: toggle and enter select mode while anything remains selected
   * - Shift+Click: contiguous range from anchor (desktop); enters select mode
   * - Long-press: enter select mode with only this item selected
   */
  function selectEntry(id: string, event: MouseEvent) {
    const additive = event.ctrlKey || event.metaKey;
    const range = event.shiftKey;

    // User has pressed Shift + Click
    if (range && selected.length > 0) {
      let anchorId = selected[0];

      if (
        selectionAnchor != null &&
        entries.some((entry) => entry.file.id === selectionAnchor)
      ) {
        anchorId = selectionAnchor;
      }

      const ids = entries.map((entry) => entry.file.id);

      const from = ids.indexOf(anchorId);
      const to = ids.indexOf(id);

      if (from !== -1 && to !== -1) {
        const start = Math.min(from, to);
        const end = Math.max(from, to);
        selected = ids.slice(start, end + 1);
        selectMode = true;
        return;
      }
    }

    // Mouse: plain click outside the current selection leaves select mode.
    if (
      selectMode &&
      !additive &&
      !range &&
      hasFinePointerHover() &&
      !selected.includes(id)
    ) {
      selected = [id];
      selectionAnchor = id;
      selectMode = false;
      return;
    }

    if (additive || selectMode) {
      toggle(id);
      selectionAnchor = id;
      selectMode = selected.length > 0;
      return;
    }

    selected = [id];
    selectionAnchor = id;
  }

  /** Long-press: enter select mode with this item as the only selection. */
  function longSelectEntry(id: string) {
    selectMode = true;
    selected = [id];
    selectionAnchor = id;
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") {
      return;
    }

    if (
      event.defaultPrevented ||
      uploadMenuOpen ||
      newMenuOpen ||
      deleteOpen ||
      shareOpen ||
      previewFileId !== null ||
      drafting
    ) {
      return;
    }

    if (!selectMode && selected.length === 0) {
      return;
    }

    event.preventDefault();

    clearSelection();
  }

  function startDelete(ids: string[]) {
    if (!canWrite || ids.length === 0) {
      return;
    }

    pendingDelete = ids.slice();
    deleteOpen = true;
  }

  function onWindowFileDragEnter(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragEnter(event);
  }

  function onWindowFileDragLeave(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragLeave(event);
  }

  function onWindowFileDragOver(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragOver(event);
  }

  function onWindowFileDrop(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDrop(event);
  }

  async function confirmDelete() {
    const ids = pendingDelete;
    const rootId = root.id;
    const parent = location.id;

    deleting = true;

    try {
      await session.client.trash(rootId, ids);

      if (root.id !== rootId || location.id !== parent) {
        return;
      }

      const gone = new Set(ids);

      entries = entries.filter((entry) => !gone.has(entry.file.id));
      pendingDelete = [];
      deleteOpen = false;
      clearSelection();
    } catch (cause) {
      const single =
        ids.length === 1
          ? entries.find((item) => item.file.id === ids[0])
          : undefined;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : single
            ? `Could not delete ${label(single)}`
            : `Could not delete ${ids.length} items`,
      );
    } finally {
      if (root.id === rootId && location.id === parent) {
        deleting = false;
      }
    }
  }

  /**
   * IDs acted on by drag or context-menu actions.
   * If the source is already selected, the whole selection is used.
   */
  function actionIds(sourceId: string): string[] {
    return selected.includes(sourceId) ? selected.slice() : [sourceId];
  }

  function dragIds(sourceId: string): string[] {
    return actionIds(sourceId);
  }

  /**
   * Right-click / action-menu target: keep a multi-selection when the
   * clicked item is already selected; otherwise select only that item.
   */
  function prepareActionTarget(id: string) {
    if (selected.includes(id)) {
      return;
    }

    selected = [id];
    selectionAnchor = id;
  }

  function dragPreview(sourceId: string): {
    entry: FileView | undefined;
    name: string;
    count: number;
  } {
    const ids = dragIds(sourceId);
    const entry = entries.find((item) => item.file.id === sourceId);

    return {
      entry,
      name: entry ? label(entry) : "Item",
      count: ids.length,
    };
  }

  async function onDragEnd(event: {
    canceled: boolean;
    operation: {
      source: { id: unknown } | null;
      target: { id: unknown } | null;
    };
  }) {
    if (event.canceled || !canWrite) {
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
      if (!showParentDrop || trail.length < 2) {
        return;
      }

      newParent = upParent;
    } else {
      const target = entries.find((entry) => entry.file.id === targetId);

      if (!target || target.file.kind !== "directory") {
        return;
      }

      newParent = targetId;
    }

    const ids = dragIds(sourceId).filter((id) => id !== targetId);

    if (ids.length === 0) {
      return;
    }

    const rootId = root.id;
    const parent = location.id;

    try {
      await session.client.move(rootId, ids, newParent);

      if (root.id !== rootId || location.id !== parent) {
        return;
      }

      const gone = new Set(ids);

      entries = entries.filter((entry) => !gone.has(entry.file.id));
      selected = selected.filter((id) => !gone.has(id));
    } catch (cause) {
      const single =
        ids.length === 1
          ? entries.find((item) => item.file.id === ids[0])
          : undefined;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : single
            ? `Could not move ${label(single)}`
            : `Could not move ${ids.length} items`,
      );
    }
  }

  function downloadName(entry: FileView) {
    return entry.file.kind === "directory"
      ? `${entry.file.name}.zip`
      : entry.file.name;
  }

  async function download(entry: FileView) {
    try {
      await session.client.download(
        root.id,
        entry.file.id,
        downloadName(entry),
      );
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${label(entry)}`,
      );
    }
  }

  /** One id uses the normal download; several become a single zip. */
  async function downloadIds(ids: string[]) {
    if (ids.length === 0) {
      return;
    }

    if (ids.length === 1) {
      const entry = entries.find((item) => item.file.id === ids[0]);

      if (entry) {
        await download(entry);
        clearSelection();
      }

      return;
    }

    try {
      await session.client.downloadDir(root.id, ids, "download.zip");
      clearSelection();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${ids.length} items`,
      );
    }
  }

  async function downloadCurrent() {
    try {
      if (locationEntry) {
        await session.client.download(
          root.id,
          locationEntry.id,
          `${locationEntry.name}.zip`,
        );
      } else {
        await session.client.downloadRoot(root.id, `${root.name}.zip`);
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${locationEntry?.name ?? root.name}`,
      );
    }
  }

  function publicLink(entry: FileView) {
    return `${window.location.origin}/files/shared/${entry.file.id}`;
  }

  function applyShare(
    id: string,
    sharingMode: SharingMode | null,
    shareExpiresAt: string | null,
  ) {
    const index = entries.findIndex((item) => item.file.id === id);

    if (index !== -1) {
      entries[index].file.sharingMode = sharingMode;
      entries[index].file.shareExpiresAt = shareExpiresAt;
    }

    if (locationEntry?.id === id) {
      locationEntry.sharingMode = sharingMode;
      locationEntry.shareExpiresAt = shareExpiresAt;
    }
  }

  /** Cascade share settings onto every visible child after sharing the open folder. */
  function setListedShare(
    sharingMode: SharingMode | null,
    shareExpiresAt: string | null,
  ) {
    for (const item of entries) {
      item.file.sharingMode = sharingMode;
      item.file.shareExpiresAt = shareExpiresAt;
    }
  }

  function openShareDialog(entry: FileEntry, ids: string[] = [entry.id]) {
    if (!canShare) {
      return;
    }

    shareTarget = entry;
    shareIds = ids.slice();
    shareOpen = true;
  }

  function openDirectoryShareDialog() {
    if (!locationEntry || !canShare) {
      return;
    }

    shareTarget = null;
    shareIds = [locationEntry.id];
    shareOpen = true;
  }

  function closeShareDialog() {
    if (shareBusy) {
      return;
    }

    shareOpen = false;
    shareTarget = null;
    shareIds = [];
  }

  async function copyLink(entry: FileView, message = "Public link copied") {
    try {
      await navigator.clipboard.writeText(publicLink(entry));
      toastSuccess(message);
    } catch {
      toastError("Could not copy the public link");
    }
  }

  async function confirmShare(input: ShareFile) {
    if (!shareSubject || shareBusy) {
      return;
    }

    const entry = shareSubject;
    const ids = shareIds.length > 0 ? shareIds.slice() : [entry.id];
    const rootId = root.id;
    const expiresAt = input.shareExpiresAt ?? null;
    const cascading = shareTarget == null;

    shareBusy = true;

    try {
      await session.client.shareFiles(rootId, ids, {
        sharingMode: input.sharingMode,
        shareExpiresAt: expiresAt,
      });

      if (root.id === rootId) {
        for (const id of ids) {
          applyShare(id, input.sharingMode, expiresAt);
        }
        if (cascading) {
          setListedShare(input.sharingMode, expiresAt);
        }
      }

      if (ids.length === 1) {
        await copyLink(
          { file: entry, metadata: null },
          `Sharing ${entry.name}`,
        );
      } else {
        toastSuccess(`Sharing ${ids.length} items`);
      }

      shareOpen = false;
      shareTarget = null;
      shareIds = [];
      clearSelection();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not create a public link",
      );
    } finally {
      if (root.id === rootId) {
        shareBusy = false;
      }
    }
  }

  async function confirmUnshare() {
    if (!shareSubject || shareBusy) {
      return;
    }

    const entry = shareSubject;
    const ids = shareIds.length > 0 ? shareIds.slice() : [entry.id];
    const rootId = root.id;
    const cascading = shareTarget == null;

    shareBusy = true;

    try {
      await session.client.unshareFiles(rootId, ids);

      if (root.id === rootId) {
        for (const id of ids) {
          applyShare(id, null, null);
        }
        if (cascading) {
          setListedShare(null, null);
        }
      }

      toastSuccess(
        ids.length === 1
          ? `Stopped sharing ${entry.name}`
          : `Stopped sharing ${ids.length} items`,
      );
      shareOpen = false;
      shareTarget = null;
      shareIds = [];
      clearSelection();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not stop sharing",
      );
    } finally {
      if (root.id === rootId) {
        shareBusy = false;
      }
    }
  }

  function startCopy(ids: string[]) {
    if (ids.length === 0) {
      return;
    }

    const destinations = writableRoots;

    if (destinations.length === 0) {
      return;
    }

    const single =
      ids.length === 1
        ? entries.find((item) => item.file.id === ids[0])
        : undefined;

    const title = single ? `Copy ${label(single)}` : `Copy ${ids.length} items`;

    const description = single
      ? `Pick a destination for ${label(single)} copy.`
      : `Pick a destination for ${ids.length} items.`;

    destination = {
      roots: destinations,
      title,
      description,
      confirmLabel: "Copy",
      onConfirm: async (targetRoot, parent) => {
        try {
          await session.client.copy(root.id, ids, targetRoot.id, parent);
          destination = null;
        } catch (cause) {
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : single
                ? `Could not copy ${label(single)}`
                : `Could not copy ${ids.length} items`,
          );
        }
      },
    };
  }

  function startMove(ids: string[]) {
    if (ids.length === 0 || !canWrite) {
      return;
    }

    const single =
      ids.length === 1
        ? entries.find((item) => item.file.id === ids[0])
        : undefined;

    const title = single ? `Move ${label(single)}` : `Move ${ids.length} items`;

    const description = single
      ? `Pick a destination for ${label(single)}.`
      : `Pick a destination for ${ids.length} items.`;

    destination = {
      roots: [root],
      title,
      description,
      confirmLabel: "Move",
      onConfirm: async (_targetRoot, parent) => {
        const rootId = root.id;
        const currentParent = location.id;

        try {
          await session.client.move(rootId, ids, parent);

          if (root.id !== rootId || location.id !== currentParent) {
            destination = null;
            return;
          }

          // Filter out all items now gone from this directory
          if (parent !== currentParent) {
            const gone = new Set(ids);
            entries = entries.filter((item) => !gone.has(item.file.id));
            selected = selected.filter((id) => !gone.has(id));
          }

          destination = null;

          selectMode = false;
        } catch (cause) {
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : single
                ? `Could not move ${label(single)}`
                : `Could not move ${ids.length} items`,
          );
        }
      },
    };
  }

  async function renameEntry(entry: FileView, name: string) {
    if (!canWrite) {
      return;
    }

    try {
      await session.client.rename(root.id, entry.file.id, name);

      const index = entries.findIndex((item) => item.file.id === entry.file.id);

      if (index !== -1) {
        entries[index].file.name = name;
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ValidationError
            ? cause.field("name")?.message || "That name is not allowed"
            : `Could not rename ${label(entry)}`,
      );
      throw cause;
    }
  }
</script>

<svelte:window
  onkeydown={onWindowKeydown}
  ondragenter={onWindowFileDragEnter}
  ondragleave={onWindowFileDragLeave}
  ondragover={onWindowFileDragOver}
  ondrop={onWindowFileDrop}
/>

<div class="relative flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  {#if uploads.fileDragActive}
    <div
      class="pointer-events-none absolute inset-0 z-50 flex items-center justify-center rounded-lg border-2 border-dashed border-border bg-card/90"
      aria-hidden="true"
    >
      <p class="text-sm font-medium text-muted-foreground">
        Drop files or folders to upload
      </p>
    </div>
  {/if}
  <div class="flex shrink-0 flex-col gap-2.5">
    <!-- PATH: always full-width above the list -->
    <div class="flex min-w-0 items-center gap-2 px-0.5">
      {#if root.owner == null}
        <RootAccessBadges accessMode={root.accessMode} />
      {/if}
      <FilePathCrumbs
        crumbs={trail}
        onNavigate={(index, crumb) => {
          ancestors = ancestors.slice(0, index);
          showDirectory(crumb.id);
        }}
      />
    </div>

    <!-- TOOLS: search + actions; stacked on small screens so menus are not clipped -->
    <div
      class="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-stretch"
    >
      <div
        class="relative min-w-0 flex-1 overflow-hidden rounded-xl border border-border bg-card shadow-sm"
      >
        <Search
          size={15}
          class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          type="search"
          class="h-9 border-0 bg-transparent pl-9 shadow-none focus-visible:ring-0"
          placeholder="Search files…"
          bind:value={searchQuery}
          aria-label="Search files in this root"
        />
      </div>

      <div
        class="relative z-10 flex shrink-0 items-center gap-0.5 self-start rounded-xl border border-border bg-card p-0.5 shadow-sm"
      >
        <input
          bind:this={fileInput}
          type="file"
          class="sr-only"
          multiple
          tabindex={-1}
          aria-hidden="true"
          onchange={(event) => uploads.fromFileInput(event)}
        />
        <input
          bind:this={folderInput}
          type="file"
          class="sr-only"
          multiple
          webkitdirectory
          tabindex={-1}
          aria-hidden="true"
          onchange={(event) => uploads.fromFolderInput(event)}
        />
        {#if selectMode}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="Clear selection"
            onclick={clearSelection}
          >
            <X size={15} />
            <span class="hidden sm:inline">Clear</span>
          </Button>
          {#if selected.length > 0}
            <span
              class="inline-flex"
              title={hasCopyDestination ? undefined : copyUnavailableTitle()}
            >
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!hasCopyDestination}
                aria-label={copyUnavailableTitle()}
                onclick={() => startCopy(selected)}
              >
                <Copy size={15} />
                <span class="hidden sm:inline">Copy ({selected.length})</span>
              </Button>
            </span>
            <span
              class="inline-flex"
              title={canWrite ? undefined : writeDeniedTitle}
            >
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!canWrite}
                aria-label={canWrite ? "Move selected" : writeDeniedTitle}
                onclick={() => startMove(selected)}
              >
                <FolderInput size={15} />
                <span class="hidden sm:inline">Move ({selected.length})</span>
              </Button>
            </span>
            <span
              class="inline-flex"
              title={canWrite ? undefined : writeDeniedTitle}
            >
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                disabled={!canWrite}
                aria-label={canWrite ? "Delete selected" : writeDeniedTitle}
                onclick={() => startDelete(selected)}
              >
                <Trash size={15} />
                <span class="hidden sm:inline">Delete ({selected.length})</span>
              </Button>
            </span>
          {/if}
        {:else}
          <Menu bind:open={uploadMenuOpen}>
            {#snippet trigger(props, open)}
              <span
                class="inline-flex"
                title={canWrite ? undefined : writeDeniedTitle}
              >
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={!canWrite}
                  aria-label={canWrite ? "Upload" : writeDeniedTitle}
                  {...props}
                  onclick={() => {
                    if (canWrite) {
                      props.onclick();
                    }
                  }}
                >
                  <Upload size={15} />
                  <span class="hidden sm:inline">Upload</span>
                  <ChevronDown
                    size={14}
                    class={[
                      "text-muted-foreground transition-transform",
                      open && "rotate-180",
                    ]}
                  />
                </Button>
              </span>
            {/snippet}
            <MenuItem onclick={() => fileInput?.click()}>
              <FileIcon size={15} />
              File
            </MenuItem>
            <MenuItem onclick={() => folderInput?.click()}>
              <Folder size={15} />
              Folder
            </MenuItem>
          </Menu>
          <Menu bind:open={newMenuOpen}>
            {#snippet trigger(props, open)}
              <span
                class="inline-flex"
                title={canWrite ? undefined : writeDeniedTitle}
              >
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={!canWrite}
                  aria-label={canWrite ? "New" : writeDeniedTitle}
                  {...props}
                  onclick={() => {
                    if (canWrite) {
                      props.onclick();
                    }
                  }}
                >
                  <Plus size={15} />
                  <span class="hidden sm:inline">New</span>
                  <ChevronDown
                    size={14}
                    class={[
                      "text-muted-foreground transition-transform",
                      open && "rotate-180",
                    ]}
                  />
                </Button>
              </span>
            {/snippet}
            <MenuItem onclick={() => beginDraft("text")}>
              <FileIcon size={15} />
              Text file
            </MenuItem>
            <MenuItem onclick={() => beginDraft("directory")}>
              <Folder size={15} />
              Directory
            </MenuItem>
          </Menu>
        {/if}
        <div
          class="mx-0.5 w-px self-stretch bg-border"
          aria-hidden="true"
        ></div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          class="text-muted-foreground"
          aria-label={locationEntry
            ? `Download ${locationEntry.name}`
            : `Download ${root.name}`}
          title={locationEntry ? "Download folder" : "Download root"}
          onclick={downloadCurrent}
        >
          <Download size={15} />
          <span class="hidden sm:inline">Download</span>
        </Button>
        {#if locationEntry}
          <div
            class="mx-0.5 w-px self-stretch bg-border"
            aria-hidden="true"
          ></div>
          <span class="inline-flex" title={directoryShareTitle()}>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class={locationEntry.sharingMode
                ? "text-primary hover:text-primary"
                : "text-muted-foreground"}
              disabled={!canShare}
              aria-label={directoryShareAriaLabel()}
              onclick={() => openDirectoryShareDialog()}
            >
              {#if locationEntry.sharingMode}
                <Link2 size={15} />
              {:else}
                <Share2 size={15} />
              {/if}
            </Button>
          </span>
        {/if}
      </div>
    </div>
  </div>

  {#if uploads.pending.length > 0}
    <ul
      class="max-h-40 shrink-0 space-y-2 overflow-y-auto rounded-lg border bg-card p-3"
    >
      {#each uploads.pending as item (item.id)}
        <PendingFileUpload
          {item}
          percent={uploads.percent(item)}
          onCancel={() => uploads.cancel(item.id)}
          onDismiss={() => uploads.dismiss(item.id)}
        />
      {/each}
    </ul>
  {/if}

  <div
    class="@container flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card"
  >
    {#if searching}
      <div
        class="flex h-10 shrink-0 items-center border-b px-3 text-xs font-medium text-muted-foreground"
      >
        Search results
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        <FileSearchResults
          {root}
          query={searchQuery}
          onNavigate={(id) => {
            searchQuery = "";
            showDirectory(id);
          }}
        />
      </div>
    {:else}
      <!-- COLUMN HEADERS -->

      {#if tiles}
        <div class="flex h-10 shrink-0 items-center justify-end border-b px-3">
          <FileViewModeMenu bind:mode={viewMode} />
        </div>
      {:else}
        <div
          class="{columns} h-10 shrink-0 border-b text-xs font-medium text-muted-foreground"
        >
          <SortHeader
            label="Name"
            column="name"
            bind:sort
            defaultSort={DEFAULT_SORT}
            onCycle={onSortCycle}
          />
          <SortHeader
            label="Size"
            column="size"
            bind:sort
            defaultSort={DEFAULT_SORT}
            onCycle={onSortCycle}
            class="hidden @lg:flex"
            buttonClass="justify-end"
          />
          <SortHeader
            label="Created"
            column="created"
            bind:sort
            defaultSort={DEFAULT_SORT}
            onCycle={onSortCycle}
            class="hidden @2xl:flex"
          />
          <div class="flex items-center justify-end">
            <FileViewModeMenu bind:mode={viewMode} />
          </div>
        </div>
      {/if}

      <!-- FILE LIST -->

      {#if loading}
        <div
          bind:this={scroller}
          class="min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <p
            class="flex h-full items-center justify-center px-3 text-sm text-muted-foreground"
          >
            Loading…
          </p>
        </div>
      {:else if entries.length === 0 && !drafting && !showParentDrop}
        <div
          bind:this={scroller}
          class="min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <div
            class="flex h-full flex-col items-center justify-center gap-2 px-6 py-16 text-center"
          >
            <Folder class="size-10 text-muted-foreground opacity-40" />
            <p class="text-sm font-medium">This folder is empty</p>
            <p class="max-w-sm text-sm text-muted-foreground">
              Upload or drop files here, or create a folder to get started.
            </p>
          </div>
        </div>
      {:else}
        {#snippet parentLeading()}
          <ParentDropItem {columns} {viewMode} onOpen={openParentDirectory} />
        {/snippet}
        <DragDropProvider {sensors} {onDragEnd}>
          <VirtualScroller
            bind:element={scroller}
            items={entries}
            itemHeight={virtualLayout.itemHeight}
            columns={virtualLayout.columns}
            gap={virtualLayout.gap}
            getKey={(entry) => entry.file.id}
            as="ul"
            class={tiles
              ? tileSize === "large"
                ? "p-2"
                : "p-1.5 sm:p-2"
              : undefined}
            windowClass={tiles ? undefined : "divide-y"}
            leading={showParentDrop ? parentLeading : undefined}
          >
            {#snippet header()}
              {#if drafting}
                <ul class={tiles ? undefined : "divide-y"}>
                  <li class={["px-3 py-2", tiles && "col-span-full"]}>
                    <div class="flex min-w-0 items-center gap-2">
                      {#if draftKind === "text"}
                        <FileIcon
                          size={18}
                          class="shrink-0 text-muted-foreground"
                        />
                      {:else}
                        <Folder size={18} class="shrink-0 text-primary" />
                      {/if}
                      <Input
                        {@attach focusDraft}
                        bind:value={directoryName}
                        class="h-8 min-w-0 flex-1 px-2.5"
                        aria-label={draftKind === "text"
                          ? "File name"
                          : "Folder name"}
                        aria-invalid={directoryError ? true : undefined}
                        autocomplete="off"
                        spellcheck="false"
                        placeholder={draftKind === "text"
                          ? "File name"
                          : "Folder name"}
                        disabled={directorySaving}
                        oninput={() => (directoryError = "")}
                        onkeydown={onDraftKey}
                      />
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label={draftKind === "text"
                          ? "Create file"
                          : "Create folder"}
                        disabled={directorySaving}
                        onclick={() => commitDirectory()}
                      >
                        <Check size={16} />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Cancel"
                        disabled={directorySaving}
                        onclick={cancelDirectory}
                      >
                        <X size={16} />
                      </Button>
                    </div>
                    <p
                      class="mt-1 pl-7 text-xs {directoryError
                        ? 'text-destructive'
                        : 'text-muted-foreground'}"
                    >
                      {directoryError ||
                        (directorySaving
                          ? "Saving…"
                          : "Enter to save, Esc to cancel")}
                    </p>
                  </li>
                </ul>
              {/if}
            {/snippet}
            {#snippet children(entry)}
              {#if tiles}
                <FileTileItem
                  {entry}
                  rootId={root.id}
                  thumbnailUrl={previews.thumbs[entry.file.id]}
                  name={label(entry)}
                  size={tileSize}
                  selected={selected.includes(entry.file.id)}
                  multiSelected={selected.length >= 2}
                  {selectMode}
                  {canWrite}
                  {canShare}
                  canCopy={hasCopyDestination}
                  onSelect={(event) => selectEntry(entry.file.id, event)}
                  onLongSelect={() => longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onPrepareAction={() => prepareActionTarget(entry.file.id)}
                  onDownload={() => downloadIds(actionIds(entry.file.id))}
                  onCopy={() => startCopy(actionIds(entry.file.id))}
                  onShare={() =>
                    openShareDialog(entry.file, actionIds(entry.file.id))}
                  onUnshare={() =>
                    openShareDialog(entry.file, actionIds(entry.file.id))}
                  onCopyLink={() => copyLink(entry)}
                  onMove={() => startMove(actionIds(entry.file.id))}
                  onDelete={() => startDelete(actionIds(entry.file.id))}
                />
              {:else}
                <FileListItem
                  {entry}
                  rootId={root.id}
                  thumbnailUrl={previews.thumbs[entry.file.id]}
                  name={label(entry)}
                  {columns}
                  density={listDensity}
                  selected={selected.includes(entry.file.id)}
                  multiSelected={selected.length >= 2}
                  {selectMode}
                  {canWrite}
                  {canShare}
                  canCopy={hasCopyDestination}
                  onSelect={(event) => selectEntry(entry.file.id, event)}
                  onLongSelect={() => longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onPrepareAction={() => prepareActionTarget(entry.file.id)}
                  onDownload={() => downloadIds(actionIds(entry.file.id))}
                  onCopy={() => startCopy(actionIds(entry.file.id))}
                  onShare={() =>
                    openShareDialog(entry.file, actionIds(entry.file.id))}
                  onUnshare={() =>
                    openShareDialog(entry.file, actionIds(entry.file.id))}
                  onCopyLink={() => copyLink(entry)}
                  onMove={() => startMove(actionIds(entry.file.id))}
                  onRename={(name) => renameEntry(entry, name)}
                  onDelete={() => startDelete(actionIds(entry.file.id))}
                />
              {/if}
            {/snippet}
          </VirtualScroller>
          <DragOverlay>
            {#snippet children(source)}
              {@const preview = dragPreview(String(source.id))}
              <div class="relative max-w-72 -rotate-1">
                {#if preview.count > 1}
                  <div
                    class="absolute inset-0 translate-x-1.5 translate-y-1.5 rounded-lg border bg-muted/80 ring-1 ring-border"
                    aria-hidden="true"
                  ></div>
                {/if}
                <div
                  class="relative flex items-center gap-2.5 rounded-lg border bg-card py-1.5 pr-2.5 pl-1.5 shadow-xl ring-1 ring-border"
                >
                  {#if preview.entry?.file.kind === "directory"}
                    <span
                      class="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
                    >
                      <Folder size={18} strokeWidth={1.75} />
                    </span>
                  {:else}
                    <span
                      class="flex size-8 shrink-0 items-center justify-center rounded-md ring-1 {FILE_ICON_CHIP_CLASS}"
                      style={fileIconChipStyle(preview.entry?.file.fileType)}
                    >
                      <FileTypeIcon
                        fileType={preview.entry?.file.fileType}
                        size={16}
                      />
                    </span>
                  {/if}
                  <span
                    class="min-w-0 flex-1 truncate text-sm font-medium"
                    title={preview.name}
                  >
                    {preview.name}
                  </span>
                  {#if preview.count > 1}
                    <span
                      class="shrink-0 rounded-full bg-primary px-2 py-0.5 text-xs font-semibold text-primary-foreground tabular-nums"
                    >
                      +{preview.count - 1}
                    </span>
                  {/if}
                </div>
              </div>
            {/snippet}
          </DragOverlay>
        </DragDropProvider>
      {/if}
    {/if}
  </div>

  {#if previewFileId !== null && previewEntries.some((entry) => entry.file.id === previewFileId)}
    <!-- 
    Use dynamic import here so we don't load preview libs (pdfjs, codemirror, sheetjs)
    on the initial file browser load
    -->
    {#await import("./FilePreview.svelte") then { default: FilePreview }}
      <FilePreview
        rootId={root.id}
        files={previewEntries}
        fileId={previewFileId}
        readonly={!canWrite}
        onClose={() => (previewFileId = null)}
        onSelect={(id) => (previewFileId = id)}
        onUpdated={(sizeBytes) => {
          const index = entries.findIndex(
            (item) => item.file.id === previewFileId,
          );
          if (index !== -1) {
            entries[index].file.sizeBytes = sizeBytes;
          }
        }}
      />
    {/await}
  {/if}

  <ConfirmDialog
    open={deleteOpen}
    title={pendingDelete.length === 1 ? "Delete item" : "Delete items"}
    description={deleteDescription}
    busy={deleting}
    onCancel={() => {
      if (!deleting) {
        deleteOpen = false;
      }
    }}
    onConfirm={confirmDelete}
  />

  {#if shareOpen && shareSubject}
    <FileShareOptions
      entry={shareSubject}
      busy={shareBusy}
      onCancel={closeShareDialog}
      onShare={confirmShare}
      onUnshare={confirmUnshare}
    />
  {/if}
</div>

<DestinationPicker
  request={destination}
  onCancel={() => (destination = null)}
/>
