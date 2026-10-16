<script lang="ts">
  import {
    Check,
    ChevronDown,
    ChevronRight,
    File as FileIcon,
    Folder,
    Plus,
    Trash,
    Upload,
    X,
  } from "@lucide/svelte";
  import { PointerActivationConstraints, PointerSensor } from "@dnd-kit/dom";
  import { DragDropProvider, DragOverlay } from "@dnd-kit/svelte";
  import { onDestroy, tick, untrack } from "svelte";
  import { navigate } from "../router";
  import { ApiError, ValidationError } from "../api";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import { type FileEntry, type FileView, type Root } from "../types";
  import { kv, type FilesViewMode } from "../storage";
  import { hasListThumbnail, isPreviewable } from "../fileKind";
  import DestinationPicker, {
    type DestinationRequest,
  } from "./DestinationPicker.svelte";
  import FileListItem from "./FileListItem.svelte";
  import FileTileItem from "./FileTileItem.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import FileViewModeMenu from "./FileViewModeMenu.svelte";
  import ParentDropItem, { PARENT_DROPPABLE_ID } from "./ParentDropItem.svelte";
  import Button from "./ui/button/button.svelte";
  import ConfirmDialog from "./ui/confirm-dialog/confirm-dialog.svelte";
  import Input from "./ui/input/input.svelte";
  import MenuItem from "./ui/menu/menu-item.svelte";
  import Menu from "./ui/menu/menu.svelte";
  import SortHeader from "./ui/sort-header/sort-header.svelte";

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

  type PendingUpload = {
    id: number;
    name: string;
    loaded: number;
    total: number;
    error: string;
  };

  /**
   * A reference to an ancestor of the folder being viewed.
   * A crumb with `id === null` represents the root directory.
   */
  type Crumb = {
    id: string | null;
    name: string;
  };

  /// How many files are sent at the same time.
  const PARALLEL_UPLOADS = 10;
  const PREVIEW_POLL_MS = 3000;
  const PREVIEW_MAX_ATTEMPTS = 5;

  type PreviewJob = { id: string; attempts: number };

  const previewQueue: PreviewJob[] = [];
  let previewThumbs = $state<Record<string, string>>({});
  let previewTimer: ReturnType<typeof setInterval> | undefined;
  let previewPolling = false;

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
  let selected = $state<string[]>([]);

  /** Anchor for Shift+click range selection (last plain / Ctrl click). */
  let selectionAnchor = $state<string | null>(null);

  /** When true, plain click/tap toggles. Entered via long-press or Shift range. */
  let selectMode = $state(false);
  let deleteOpen = $state(false);
  let deleting = $state(false);
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

  let pending = $state<PendingUpload[]>([]);

  let nextUploadId = 0;

  /// Queued and in-flight uploads. Aborting one cancels that file only.
  const uploadControllers = new Map<number, AbortController>();

  /** Desktop OS file drag overlay; depth avoids flicker across child elements. */
  let fileDragActive = $state(false);
  let fileDragDepth = 0;

  let uploadMenuOpen = $state(false);
  let newMenuOpen = $state(false);

  let ancestors = $state<Crumb[]>([]);

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

  /** Parent of the current folder; null when moving into the root. */
  const upParent = $derived(
    trail.length >= 2 ? trail[trail.length - 2].id : null,
  );

  const showParentDrop = $derived(location.id !== null);

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
      void open(rootId, parent);
    });
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
    // If the last ancestor is the current parent (dir), we're there
    // (but only if the trail has unique ids)
    const tipMatches = (ancestors.at(-1)?.id ?? null) === parent;

    const uniqueIds =
      new Set(ancestors.map((a) => a.id)).size === ancestors.length;

    if (tipMatches && uniqueIds) {
      return;
    }

    if (parent === null) {
      ancestors = [];
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
    } catch (cause) {
      if (currentLoad !== latestLoad) {
        return;
      }

      ancestors = [];

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

  function stopPreviewPoll() {
    if (previewTimer !== undefined) {
      clearInterval(previewTimer);
      previewTimer = undefined;
    }
  }

  function clearPreviewThumbs() {
    stopPreviewPoll();

    previewQueue.length = 0;

    for (const url of Object.values(previewThumbs)) {
      URL.revokeObjectURL(url);
    }

    previewThumbs = {};
  }

  function enqueuePreview(file: FileEntry) {
    if (file.kind !== "file" || file.parent !== location.id) {
      return;
    }

    if (!hasListThumbnail({ file, metadata: null })) {
      return;
    }

    if (
      previewThumbs[file.id] ||
      previewQueue.some((job) => job.id === file.id)
    ) {
      return;
    }

    previewQueue.push({ id: file.id, attempts: 0 });

    startPreviewPoll();
  }

  function startPreviewPoll() {
    if (previewTimer !== undefined) {
      return;
    }

    void drainPreviewQueue();

    previewTimer = setInterval(() => {
      void drainPreviewQueue();
    }, PREVIEW_POLL_MS);
  }

  async function drainPreviewQueue() {
    if (previewPolling || previewQueue.length === 0) {
      if (previewQueue.length === 0) {
        stopPreviewPoll();
      }
      return;
    }

    previewPolling = true;

    const rootId = root.id;
    // Tiles (small or large) need 256; list icons stay at 64.
    const previewSize = tiles ? 256 : 64;
    const jobs = previewQueue.slice();

    try {
      await Promise.all(
        jobs.map(async (job) => {
          try {
            const blob = await session.client.preview(
              rootId,
              job.id,
              previewSize,
            );

            const prev = previewThumbs[job.id];

            if (prev) {
              URL.revokeObjectURL(prev);
            }

            previewThumbs[job.id] = URL.createObjectURL(blob);

            const index = previewQueue.findIndex((item) => item.id === job.id);

            if (index !== -1) {
              previewQueue.splice(index, 1);
            }
          } catch {
            job.attempts += 1;
            if (job.attempts >= PREVIEW_MAX_ATTEMPTS) {
              const index = previewQueue.findIndex(
                (item) => item.id === job.id,
              );
              if (index !== -1) {
                previewQueue.splice(index, 1);
              }
            }
          }
        }),
      );
    } finally {
      previewPolling = false;
      if (previewQueue.length === 0) {
        stopPreviewPoll();
      }
    }
  }

  // Swap cached thumbs when switching between list (64) and tiles (256).
  let seenTiles: boolean | undefined;
  $effect(() => {
    const next = tiles;
    if (seenTiles !== undefined && seenTiles !== next) {
      for (const url of Object.values(previewThumbs)) {
        URL.revokeObjectURL(url);
      }
      previewThumbs = {};
      for (const entry of entries) {
        enqueuePreview(entry.file);
      }
    }
    seenTiles = next;
  });

  onDestroy(clearPreviewThumbs);

  async function open(rootId: string, parent: string | null) {
    const currentLoad = ++latestLoad;

    resetDraft();

    loading = true;

    clearPreviewThumbs();

    entries = [];
    selected = [];
    selectionAnchor = null;
    selectMode = false;
    previewFileId = null;

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

  function cycleSort(column: SortColumn) {
    if (sort.column !== column) {
      sort = { column, direction: "asc" };
    } else if (sort.direction === "asc") {
      sort = { column, direction: "desc" };
    } else {
      sort = { ...DEFAULT_SORT };
    }

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

  function uploadFailure(item: PendingUpload, cause: unknown) {
    uploadControllers.delete(item.id);

    if (cause instanceof DOMException && cause.name === "AbortError") {
      removeUpload(item.id);
      return;
    }

    if (cause instanceof TypeError) {
      item.error = "The server could not be reached";
    } else if (cause instanceof ApiError && cause.status === 413) {
      item.error = "Not enough space";
    } else if (cause instanceof ApiError && cause.status === 403) {
      item.error = "You cannot upload here";
    } else {
      item.error = "Upload failed";
    }
  }

  async function uploadFiles(files: File[]) {
    if (files.length === 0) {
      return;
    }

    const rootId = root.id;

    const parent = location.id;

    const queue = files.map((file) => {
      const id = nextUploadId++;
      const controller = new AbortController();

      uploadControllers.set(id, controller);

      pending.push({
        id,
        name: file.name,
        loaded: 0,
        total: file.size,
        error: "",
      });

      // Read the element back so later writes go through the reactive proxy.
      const item = pending[pending.length - 1];

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
            const created = await session.client.upload(
              rootId,
              parent,
              file,
              (loaded, total) => {
                item.loaded = loaded;
                item.total = total;
              },
              controller.signal,
            );

            removeUpload(item.id);

            if (root.id === rootId && location.id === parent) {
              insertEntry(created);
              enqueuePreview(created);
            }
          } catch (cause) {
            uploadFailure(item, cause);
          }
        }
      },
    );

    await Promise.all(workers);
  }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;

    const files: File[] = [];

    if (input.files) {
      for (const file of input.files) {
        files.push(file);
      }
    }

    input.value = "";

    await uploadFiles(files);
  }

  async function uploadFolderFiles(files: File[]) {
    if (files.length === 0) {
      return;
    }

    const rootId = root.id;
    const parent = location.id;
    const id = nextUploadId++;
    const controller = new AbortController();

    uploadControllers.set(id, controller);

    const path = files[0]?.webkitRelativePath || files[0]?.name || "Folder";
    const name = path.split("/")[0] || "Folder";
    const total = files.reduce((sum, file) => sum + file.size, 0);

    pending.push({
      id,
      name,
      loaded: 0,
      total,
      error: "",
    });

    const item = pending[pending.length - 1];

    try {
      const created = await session.client.uploadDir(
        rootId,
        parent,
        files,
        (loaded, totalBytes) => {
          item.loaded = loaded;
          item.total = totalBytes;
        },
        controller.signal,
      );

      removeUpload(item.id);

      if (root.id === rootId && location.id === parent) {
        for (const entry of created) {
          insertEntry(entry);
          enqueuePreview(entry);
        }
      }
    } catch (cause) {
      uploadFailure(item, cause);
    }
  }

  async function uploadFolder(event: Event) {
    const input = event.currentTarget as HTMLInputElement;

    const files: File[] = [];

    if (input.files) {
      for (const file of input.files) {
        files.push(file);
      }
    }

    input.value = "";

    await uploadFolderFiles(files);
  }

  function isExternalFileDrag(event: DragEvent) {
    return Boolean(event.dataTransfer?.types.includes("Files"));
  }

  function supportsExternalFileDrop() {
    return window.matchMedia("(pointer: fine)").matches;
  }

  function onWindowDragEnter(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    fileDragDepth += 1;
    fileDragActive = true;
  }

  function onWindowDragLeave(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    fileDragDepth = Math.max(0, fileDragDepth - 1);

    if (fileDragDepth === 0) {
      fileDragActive = false;
    }
  }

  function onWindowDragOver(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "copy";
    }
  }

  async function onWindowDrop(event: DragEvent) {
    if (!supportsExternalFileDrop() || !isExternalFileDrag(event)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    fileDragDepth = 0;
    fileDragActive = false;

    const batch = await collectDropped(event.dataTransfer);
    const uploads = [uploadFiles(batch.files)];

    for (const files of batch.directories) {
      uploads.push(uploadFolderFiles(files));
    }

    await Promise.all(uploads);
  }

  /**
   * Directory members are `File`s whose `name` is the relative path so
   * `uploadDir` can recreate the tree via `webkitRelativePath || name`.
   */
  async function collectDropped(dataTransfer: DataTransfer | null) {
    const files: File[] = [];
    const directories: File[][] = [];

    if (!dataTransfer) {
      return { files, directories };
    }

    const items = dataTransfer.items;

    if (
      items &&
      items.length > 0 &&
      typeof items[0].webkitGetAsEntry === "function"
    ) {
      for (let i = 0; i < items.length; i++) {
        const entry = items[i].webkitGetAsEntry();
        if (!entry) {
          continue;
        }

        if (entry.isFile) {
          files.push(await readFileEntry(entry as FileSystemFileEntry));
        } else if (entry.isDirectory) {
          const dirFiles = await readDirectoryEntry(
            entry as FileSystemDirectoryEntry,
            entry.name,
          );
          if (dirFiles.length > 0) {
            directories.push(dirFiles);
          }
        }
      }

      return { files, directories };
    }

    for (let i = 0; i < dataTransfer.files.length; i++) {
      files.push(dataTransfer.files[i]);
    }

    return { files, directories };
  }

  function readFileEntry(entry: FileSystemFileEntry) {
    return new Promise<File>((resolve, reject) => {
      entry.file(resolve, reject);
    });
  }

  function readAllEntries(reader: FileSystemDirectoryReader) {
    return new Promise<FileSystemEntry[]>((resolve, reject) => {
      const all: FileSystemEntry[] = [];

      const readBatch = () => {
        reader.readEntries((batch) => {
          if (batch.length === 0) {
            resolve(all);
            return;
          }
          for (const entry of batch) {
            all.push(entry);
          }
          readBatch();
        }, reject);
      };

      readBatch();
    });
  }

  async function readDirectoryEntry(
    entry: FileSystemDirectoryEntry,
    prefix: string,
  ) {
    const children = await readAllEntries(entry.createReader());
    const files: File[] = [];

    for (const child of children) {
      if (child.isFile) {
        const file = await readFileEntry(child as FileSystemFileEntry);

        files.push(new File([file], `${prefix}/${file.name}`));
      } else if (child.isDirectory) {
        const nested = await readDirectoryEntry(
          child as FileSystemDirectoryEntry,
          `${prefix}/${child.name}`,
        );

        for (const file of nested) {
          files.push(file);
        }
      }
    }

    return files;
  }

  function removeUpload(id: number) {
    uploadControllers.delete(id);

    const index = pending.findIndex((entry) => entry.id === id);

    if (index !== -1) {
      pending.splice(index, 1);
    }
  }

  function cancelUpload(id: number) {
    uploadControllers.get(id)?.abort();
    removeUpload(id);
  }

  function percent(item: PendingUpload) {
    if (item.total <= 0) {
      return 0;
    }
    return Math.min(100, Math.round((item.loaded / item.total) * 100));
  }

  function focusDraft(node: HTMLInputElement) {
    nameInput = node;

    queueMicrotask(() => {
      node.focus();
      node.select();
    });

    return () => {
      if (nameInput === node) {
        nameInput = undefined;
      }
    };
  }

  async function beginDraft(kind: "directory" | "text") {
    if (drafting) {
      nameInput?.focus();
      nameInput?.select();
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
      void commitDirectory();
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

  async function commitDirectory() {
    if (!draftKind || directorySaving) {
      return;
    }

    const kind = draftKind;
    const name = directoryName.trim();

    if (!name) {
      directoryError =
        kind === "text"
          ? "File name cannot be empty"
          : "Directory name cannot be empty";
      nameInput?.focus();
      return;
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

  function startMove(entry: FileView) {
    destination = {
      roots: [root],
      title: `Move ${label(entry)}`,
      description: `Pick a destination for ${label(entry)}.`,
      confirmLabel: "Move",
      onConfirm: async (_targetRoot, parent) => {
        const rootId = root.id;
        const currentParent = location.id;

        try {
          await session.client.move(rootId, [entry.file.id], parent);

          if (root.id !== rootId || location.id !== currentParent) {
            destination = null;
            return;
          }

          if (parent !== currentParent) {
            const gone = new Set([entry.file.id]);
            entries = entries.filter((item) => !gone.has(item.file.id));
            selected = selected.filter((id) => !gone.has(id));
          }

          destination = null;
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
      entries = entries.map((item) =>
        item.file.id === entry.file.id
          ? { ...item, file: { ...item.file, name } }
          : item,
      );
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
  ondragenter={onWindowDragEnter}
  ondragleave={onWindowDragLeave}
  ondragover={onWindowDragOver}
  ondrop={onWindowDrop}
/>

<div class="relative flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  {#if fileDragActive}
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
    <nav
      class="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto text-sm"
      aria-label="Current folder"
    >
      {#each trail as crumb, index (crumb.id ?? "root")}
        {#if index > 0}
          <ChevronRight
            size={14}
            class="shrink-0 text-muted-foreground"
            aria-hidden="true"
          />
        {/if}

        {#if index === trail.length - 1}
          <span
            class="max-w-40 truncate px-1.5 py-1 font-medium sm:max-w-none"
            aria-current="page"
          >
            {crumb.name}
          </span>
        {:else}
          <button
            type="button"
            class="max-w-40 truncate rounded-md px-1.5 py-1 text-muted-foreground hover:bg-accent hover:text-foreground sm:max-w-none"
            onclick={() => {
              ancestors = ancestors.slice(0, index);
              showDirectory(crumb.id);
            }}
          >
            {crumb.name}
          </button>
        {/if}
      {/each}
    </nav>

    <div class="flex shrink-0 items-center gap-1.5">
      <input
        bind:this={fileInput}
        type="file"
        class="sr-only"
        multiple
        tabindex={-1}
        aria-hidden="true"
        onchange={upload}
      />
      <input
        bind:this={folderInput}
        type="file"
        class="sr-only"
        multiple
        webkitdirectory
        tabindex={-1}
        aria-hidden="true"
        onchange={uploadFolder}
      />
      <!-- 
           Native OS file pickers (at least linux) include both files
           and directories when requesting with webkitdirectory
           so there is no need for selection
      -->
      {#if session.native}
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-label="Upload"
          onclick={() => folderInput?.click()}
        >
          <Upload size={15} />
          <span class="hidden sm:inline">Upload</span>
        </Button>
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
      {/if}
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
        <MenuItem onclick={() => void beginDraft("text")}>
          <FileIcon size={15} />
          Text file
        </MenuItem>
        <MenuItem onclick={() => void beginDraft("directory")}>
          <Folder size={15} />
          Directory
        </MenuItem>
      </Menu>
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
      {/if}
      {#if selectMode && selected.length > 0}
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
    </div>
  </div>

  {#if pending.length > 0}
    <ul
      class="max-h-40 shrink-0 space-y-2 overflow-y-auto rounded-lg border bg-card p-3"
    >
      {#each pending as item (item.id)}
        <li class="space-y-1">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span class="min-w-0 truncate">{item.name}</span>
            {#if item.error}
              <span class="flex shrink-0 items-center gap-2 text-destructive">
                {item.error}
                <button
                  type="button"
                  class="text-xs underline"
                  onclick={() =>
                    (pending = pending.filter((entry) => entry.id !== item.id))}
                >
                  Dismiss
                </button>
              </span>
            {:else}
              <span class="flex shrink-0 items-center gap-2">
                <span class="text-muted-foreground tabular-nums"
                  >{percent(item)}%</span
                >
                <button
                  type="button"
                  class="text-xs underline"
                  aria-label="Cancel {item.name}"
                  onclick={() => cancelUpload(item.id)}
                >
                  Cancel
                </button>
              </span>
            {/if}
          </div>
          <div
            class="h-1.5 overflow-hidden rounded-full bg-muted"
            role="progressbar"
            aria-label={item.name}
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={percent(item)}
          >
            <div
              class="h-full rounded-full transition-[width] duration-150 {item.error
                ? 'bg-destructive'
                : 'bg-primary'}"
              style:width="{percent(item)}%"
            ></div>
          </div>
        </li>
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
        {@render columnHeader("name", "Name")}
        {@render columnHeader("size", "Size", "hidden @lg:flex", "justify-end")}
        {@render columnHeader("created", "Created", "hidden @2xl:flex")}
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
                  thumbnailUrl={previewThumbs[entry.file.id]}
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
                  onDelete={() => startDelete([entry.file.id])}
                />
              {:else}
                <FileListItem
                  {entry}
                  rootId={root.id}
                  thumbnailUrl={previewThumbs[entry.file.id]}
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
                  onMove={() => startMove(entry)}
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
    {#await import("./FilePreview.svelte") then { default: FilePreview }}
      <FilePreview
        rootId={root.id}
        files={previewEntries}
        fileId={previewFileId}
        onClose={() => (previewFileId = null)}
        onSelect={(id) => (previewFileId = id)}
        onUpdated={(entry) => {
          entries = entries.map((item) =>
            item.file.id === entry.id ? { file: entry, metadata: null } : item,
          );
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
</div>

<DestinationPicker
  request={destination}
  onCancel={() => (destination = null)}
/>

{#snippet columnHeader(
  column: SortColumn,
  label: string,
  cellClass?: string,
  buttonClass?: string,
)}
  <SortHeader
    {label}
    direction={sort.column === column ? sort.direction : undefined}
    onclick={() => cycleSort(column)}
    class={cellClass}
    {buttonClass}
  />
{/snippet}
