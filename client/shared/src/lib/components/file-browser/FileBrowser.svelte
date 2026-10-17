<script lang="ts">
  import {
    Check,
    ChevronDown,
    File as FileIcon,
    Folder,
    FolderInput,
    Link as LinkIcon,
    Link2,
    Link2Off,
    Plus,
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
  import { type FileEntry, type FileView, type Root } from "../../types";
  import { kv, type FilesViewMode } from "../../storage";
  import { UploadHandler } from "../../uploadHandler.svelte";
  import { isPreviewable } from "../../fileKind";
  import DestinationPicker, {
    type DestinationRequest,
  } from "./DestinationPicker.svelte";
  import FileListItem from "./FileListItem.svelte";
  import FilePathCrumbs, { type Crumb } from "./FilePathCrumbs.svelte";
  import FileTileItem from "./FileTileItem.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import FileViewModeMenu from "./FileViewModeMenu.svelte";
  import ParentDropItem, { PARENT_DROPPABLE_ID } from "./ParentDropItem.svelte";
  import PendingFileUpload from "./PendingFileUpload.svelte";
  import Button from "../ui/button/button.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import Dialog from "../ui/dialog/dialog.svelte";
  import Input from "../ui/input/input.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import Menu from "../ui/menu/menu.svelte";
  import SortHeader from "../ui/sort-header/sort-header.svelte";

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

  /**
   * Empty on component load, but after a user clicks a list item
   * always has at least one entry until they have deleted/moved it
   * in which case it becomes empty again.
   * When this has length >= 2 the multiselect context for deleting
   * and moving appears.
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

  const tiles = $derived(
    viewMode === "tiles-small" || viewMode === "tiles-large",
  );

  const tileSize = $derived<"small" | "large">(
    viewMode === "tiles-large" ? "large" : "small",
  );

  const listDensity = $derived<"default" | "compact">(
    viewMode === "compact" ? "compact" : "default",
  );

  const tileGridClass = $derived(
    viewMode === "tiles-large"
      ? "grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-1 p-2 sm:gap-2"
      : "grid grid-cols-[repeat(auto-fill,minmax(6.25rem,1fr))] gap-0.5 p-1.5 sm:gap-1 sm:p-2",
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

  let shareDirOpen = $state(false);
  let shareDirBusy = $state(false);

  let fileInput = $state<HTMLInputElement | undefined>();
  let folderInput = $state<HTMLInputElement | undefined>();

  let scroller = $state<HTMLDivElement | undefined>();

  let nameInput: HTMLInputElement | undefined;

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

  const shareDirDescription = $derived(
    locationEntry?.shared
      ? "This folder is publicly shared. Anyone with the link can view it and its contents."
      : "Create a public link so anyone can view this folder and its contents.",
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
    const next = tiles;

    if (seenTiles !== undefined && seenTiles !== next) {
      previews.refreshAll(entries.map((entry) => entry.file));
    }

    seenTiles = next;
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
    shareDirOpen = false;
    shareDirBusy = false;

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
      shareDirOpen ||
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
    pendingDelete = ids.slice();
    deleteOpen = true;
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
      selected = selected.filter((id) => !gone.has(id));

      pendingDelete = [];

      deleteOpen = false;
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not delete that item",
      );
    } finally {
      if (root.id === rootId && location.id === parent) {
        deleting = false;
      }
    }
  }

  function dragIds(sourceId: string): string[] {
    return selected.includes(sourceId) ? selected.slice() : [sourceId];
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
    if (event.canceled) {
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
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not move that item",
      );
    }
  }

  async function download(entry: FileView) {
    try {
      await session.client.download(root.id, entry.file.id, entry.file.name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not download that file",
      );
    }
  }

  function publicLink(entry: FileView) {
    return `${window.location.origin}/files/shared/${entry.file.id}`;
  }

  function setShared(id: string, shared: boolean) {
    const index = entries.findIndex((item) => item.file.id === id);

    if (index !== -1) {
      entries[index].file.shared = shared;
    }

    if (locationEntry?.id === id) {
      locationEntry.shared = shared;
    }
  }

  async function copyLink(entry: FileView, message = "Public link copied") {
    try {
      await navigator.clipboard.writeText(publicLink(entry));
      toastSuccess(message);
    } catch {
      toastError("Could not copy the public link");
    }
  }

  async function copyLocationLink(message = "Public link copied") {
    if (!locationEntry) {
      return;
    }

    await copyLink({ file: locationEntry, metadata: null }, message);
  }

  async function shareEntry(entry: FileView) {
    const rootId = root.id;

    try {
      await session.client.shareFile(rootId, entry.file.id);

      // Only the opened entry is listed here. Descendants of a shared
      // directory are not part of this list, so no cascade is needed.
      if (root.id === rootId) {
        setShared(entry.file.id, true);
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not create a public link",
      );
      return;
    }

    await copyLink(entry, `Sharing ${entry.file.name}`);
  }

  async function unshareEntry(entry: FileView) {
    const rootId = root.id;

    try {
      await session.client.unshareFile(rootId, entry.file.id);

      if (root.id === rootId) {
        setShared(entry.file.id, false);
      }

      toastSuccess(`Stopped sharing ${entry.file.name}`);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not stop sharing",
      );
    }
  }

  /** Cascade share flag onto every visible child after sharing the open folder. */
  function setListedShared(shared: boolean) {
    for (const item of entries) {
      item.file.shared = shared;
    }
  }

  async function shareCurrentDirectory() {
    if (!locationEntry || shareDirBusy) {
      return;
    }

    const entry = locationEntry;
    const rootId = root.id;

    shareDirBusy = true;

    try {
      await session.client.shareFile(rootId, entry.id);

      if (root.id === rootId && locationEntry?.id === entry.id) {
        locationEntry.shared = true;
        setListedShared(true);
      }

      await copyLocationLink(`Sharing ${entry.name}`);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not create a public link",
      );
    } finally {
      if (root.id === rootId) {
        shareDirBusy = false;
      }
    }
  }

  async function unshareCurrentDirectory() {
    if (!locationEntry || shareDirBusy) {
      return;
    }

    const entry = locationEntry;
    const rootId = root.id;

    shareDirBusy = true;

    try {
      await session.client.unshareFile(rootId, entry.id);

      if (root.id === rootId && locationEntry?.id === entry.id) {
        locationEntry.shared = false;
        setListedShared(false);
      }

      toastSuccess(`Stopped sharing ${entry.name}`);
      shareDirOpen = false;
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not stop sharing",
      );
    } finally {
      if (root.id === rootId) {
        shareDirBusy = false;
      }
    }
  }

  function startCopy(entry: FileView) {
    const destinations = roots.filter((item) => item.id !== root.id);

    if (destinations.length === 0) {
      toastError("No other roots available");
      return;
    }

    destination = {
      roots: destinations,
      title: `Copy ${label(entry)}`,
      description: `Pick a destination for ${label(entry)} copy.`,
      confirmLabel: "Copy",
      onConfirm: async (targetRoot, parent) => {
        try {
          await session.client.copy(
            root.id,
            entry.file.id,
            targetRoot.id,
            parent,
          );
          destination = null;
        } catch (cause) {
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : "Could not copy that item",
          );
        }
      },
    };
  }

  function startMove(ids: string[]) {
    if (ids.length === 0) {
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
              : "Could not move that item",
          );
        }
      },
    };
  }

  async function renameEntry(entry: FileView, name: string) {
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
            : "Could not rename that item",
      );
      throw cause;
    }
  }
</script>

<svelte:window
  onkeydown={onWindowKeydown}
  ondragenter={(event) => uploads.onWindowDragEnter(event)}
  ondragleave={(event) => uploads.onWindowDragLeave(event)}
  ondragover={(event) => uploads.onWindowDragOver(event)}
  ondrop={(event) => uploads.onWindowDrop(event)}
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
  <div class="flex shrink-0 flex-wrap items-center justify-between gap-2">
    <FilePathCrumbs
      crumbs={trail}
      onNavigate={(index, crumb) => {
        ancestors = ancestors.slice(0, index);
        showDirectory(crumb.id);
      }}
    />

    <div class="flex shrink-0 items-center gap-1.5">
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
          variant="outline"
          size="sm"
          aria-label="Clear selection"
          onclick={clearSelection}
        >
          <X size={15} />
          <span class="hidden sm:inline">Clear</span>
        </Button>
        {#if selected.length > 0}
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-label="Move selected"
            onclick={() => startMove(selected)}
          >
            <FolderInput size={15} />
            <span class="hidden sm:inline">Move ({selected.length})</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            class="text-destructive hover:bg-destructive/10 hover:text-destructive"
            aria-label="Delete selected"
            onclick={() => startDelete(selected)}
          >
            <Trash size={15} />
            <span class="hidden sm:inline">Delete ({selected.length})</span>
          </Button>
        {/if}
      {:else}
        <Menu bind:open={uploadMenuOpen}>
          {#snippet trigger(props, open)}
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Upload"
              {...props}
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
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="New"
              {...props}
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
      {#if locationEntry}
        <Button
          type="button"
          variant="outline"
          size="sm"
          class={locationEntry.shared
            ? "text-primary hover:text-primary"
            : "text-muted-foreground"}
          aria-label={locationEntry.shared
            ? "Folder is shared — manage sharing"
            : "Share this folder"}
          title={locationEntry.shared ? "Shared" : "Share folder"}
          onclick={() => (shareDirOpen = true)}
        >
          {#if locationEntry.shared}
            <Link2 size={15} />
          {:else}
            <Share2 size={15} />
          {/if}
        </Button>
      {/if}
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

    <div
      bind:this={scroller}
      class="min-h-0 flex-1 overflow-y-auto overscroll-contain"
    >
      {#if loading}
        <p
          class="flex h-full items-center justify-center px-3 text-sm text-muted-foreground"
        >
          Loading…
        </p>
      {:else if entries.length === 0 && !drafting && !showParentDrop}
        <div
          class="flex h-full flex-col items-center justify-center gap-2 px-6 py-16 text-center"
        >
          <Folder class="size-10 text-muted-foreground opacity-40" />
          <p class="text-sm font-medium">This folder is empty</p>
          <p class="max-w-sm text-sm text-muted-foreground">
            Upload or drop files here, or create a folder to get started.
          </p>
        </div>
      {:else}
        <DragDropProvider {sensors} {onDragEnd}>
          <ul class={tiles ? tileGridClass : "divide-y"}>
            {#if showParentDrop}
              <ParentDropItem
                {columns}
                {viewMode}
                onOpen={openParentDirectory}
              />
            {/if}

            <!-- NEW DIRECTORY DRAFT -->

            {#if drafting}
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
            {/if}

            <!-- FILES -->

            {#each entries as entry (entry.file.id)}
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
                  onSelect={(event) => selectEntry(entry.file.id, event)}
                  onLongSelect={() => longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onDownload={() => download(entry)}
                  onCopy={() => startCopy(entry)}
                  onShare={() => shareEntry(entry)}
                  onUnshare={() => unshareEntry(entry)}
                  onCopyLink={() => copyLink(entry)}
                  onDelete={() => startDelete([entry.file.id])}
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
                  onSelect={(event) => selectEntry(entry.file.id, event)}
                  onLongSelect={() => longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onDownload={() => download(entry)}
                  onCopy={() => startCopy(entry)}
                  onShare={() => shareEntry(entry)}
                  onUnshare={() => unshareEntry(entry)}
                  onCopyLink={() => copyLink(entry)}
                  onMove={() => startMove([entry.file.id])}
                  onRename={(name) => renameEntry(entry, name)}
                  onDelete={() => startDelete([entry.file.id])}
                />
              {/if}
            {/each}
          </ul>
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
                      class="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground ring-1 ring-border/60"
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
    </div>
  </div>

  {#if previewFileId !== null && previewEntries.some((entry) => entry.file.id === previewFileId)}
    <!-- 
    Use dymanic import here so we don't load preview libs (pdfjs, codemirror)
    on the initial file browser load
    -->
    {#await import("./FilePreview.svelte") then { default: FilePreview }}
      <FilePreview
        rootId={root.id}
        files={previewEntries}
        fileId={previewFileId}
        onClose={() => (previewFileId = null)}
        onSelect={(id) => (previewFileId = id)}
        onUpdated={(entry) => {
          const index = entries.findIndex((item) => item.file.id === entry.id);

          if (index !== -1) {
            entries[index] = { file: entry, metadata: null };
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

  {#if locationEntry}
    <Dialog
      open={shareDirOpen}
      title={locationEntry.shared ? "Shared folder" : "Share folder"}
      description={shareDirDescription}
      busy={shareDirBusy}
      onCancel={() => {
        if (!shareDirBusy) {
          shareDirOpen = false;
        }
      }}
    >
      {#snippet footer()}
        <Button
          type="button"
          variant="outline"
          onclick={() => (shareDirOpen = false)}
          disabled={shareDirBusy}
        >
          Close
        </Button>
        {#if locationEntry.shared}
          <Button
            type="button"
            variant="outline"
            onclick={() => copyLocationLink()}
            disabled={shareDirBusy}
          >
            <LinkIcon size={15} />
            Copy link
          </Button>
          <Button
            type="button"
            variant="destructive"
            onclick={() => unshareCurrentDirectory()}
            disabled={shareDirBusy}
          >
            <Link2Off size={15} />
            Stop sharing
          </Button>
        {:else}
          <Button
            type="button"
            onclick={() => shareCurrentDirectory()}
            disabled={shareDirBusy}
          >
            <Share2 size={15} />
            Create public link
          </Button>
        {/if}
      {/snippet}
    </Dialog>
  {/if}
</div>

<DestinationPicker
  request={destination}
  onCancel={() => (destination = null)}
/>
