<script lang="ts">
  import {
    ArrowDown,
    ArrowUp,
    Check,
    ChevronRight,
    Folder,
    FolderPlus,
    Settings,
    Trash,
    Upload,
    X,
  } from "@lucide/svelte";
  import { tick, untrack } from "svelte";
  import { navigate } from "../router";
  import { ApiError, ValidationError } from "../api";
  import { formatQuota, formatDatetime } from "../format";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import { type FileEntry, type FileView, type Root } from "../types";
  import { cn } from "../utils";
  import FileListItem from "./FileListItem.svelte";
  import ImagePreview from "./ImagePreview.svelte";
  import Button from "./ui/button/button.svelte";
  import ConfirmDialog from "./ui/confirm-dialog/confirm-dialog.svelte";

  let {
    folder: root,
    parentId = null,
    admin = false,
  }: {
    folder: Root;
    /** Directory open in this root. Null lists the root itself. */
    parentId?: string | null;
    admin?: boolean;
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

  type SortColumn = "kind" | "name" | "size" | "created" | "modified";
  type SortDirection = "asc" | "desc";

  const columns =
    "grid grid-cols-[1.75rem_minmax(0,1fr)_5.5rem] items-center gap-x-3 px-3 @lg:grid-cols-[1.75rem_minmax(8rem,1fr)_minmax(0,5.5rem)_5.5rem] @2xl:grid-cols-[1.75rem_minmax(8rem,1fr)_minmax(0,5.5rem)_minmax(0,11.5rem)_5.5rem] @4xl:grid-cols-[1.75rem_minmax(8rem,1fr)_minmax(0,5.5rem)_minmax(0,11.5rem)_minmax(0,11.5rem)_5.5rem]";

  let entries = $state<FileView[]>([]);
  let selected = $state<string[]>([]);
  let deleteOpen = $state(false);
  let deleting = $state(false);
  let pendingDelete = $state<string[]>([]);
  let previewFileId = $state<string | null>(null);

  const imageEntries = $derived(
    entries.filter(
      (entry) =>
        entry.file.kind === "file" &&
        entry.file.fileType != null &&
        "image" in entry.file.fileType,
    ),
  );
  let sort = $state<{ column: SortColumn; direction: SortDirection }>({
    column: "kind",
    direction: "asc",
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

  let drafting = $state(false);

  let directoryName = $state("");
  let directoryError = $state("");
  let directorySaving = $state(false);

  let pending = $state<PendingUpload[]>([]);

  let nextUploadId = 0;

  /// Queued and in-flight uploads. Aborting one cancels that file only.
  const uploadControllers = new Map<number, AbortController>();

  let ancestors = $state<Crumb[]>([]);

  let fileInput = $state<HTMLInputElement | undefined>();

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
    if (ancestors.at(-1)?.id ?? null === parent) {
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
    drafting = false;
    directorySaving = false;
    directoryName = "";
    directoryError = "";
  }

  async function open(rootId: string, parent: string | null) {
    const currentLoad = ++latestLoad;

    resetDraft();

    loading = true;

    entries = [];
    selected = [];
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
      sort = { column: "kind", direction: "asc" };
    }

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
        case "modified":
          byColumn = a.updatedAt.localeCompare(b.updatedAt);
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

  function ariaSort(column: SortColumn): "ascending" | "descending" | "none" {
    if (sort.column !== column) {
      return "none";
    }
    return sort.direction === "asc" ? "ascending" : "descending";
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
    ancestors = ancestors.concat({
      id: entry.file.id,
      name: entry.file.name,
    });
    showDirectory(entry.file.id);
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
            }
          } catch (cause) {
            uploadControllers.delete(item.id);

            if (cause instanceof DOMException && cause.name === "AbortError") {
              removeUpload(item.id);
              continue;
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
        }
      },
    );

    await Promise.all(workers);
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

    return {
      destroy() {
        if (nameInput === node) {
          nameInput = undefined;
        }
      },
    };
  }

  async function beginDirectory() {
    if (drafting) {
      nameInput?.focus();
      nameInput?.select();
      return;
    }

    directoryName = "New folder";
    directoryError = "";
    directorySaving = false;

    drafting = true;

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
    if (cause instanceof ApiError && cause.status === 403) {
      return "You cannot create a folder here";
    }
    if (cause instanceof ApiError && cause.status === 422) {
      return "That folder name is not allowed";
    }
    return "Could not create the folder";
  }

  async function commitDirectory() {
    if (!drafting || directorySaving) {
      return;
    }

    const name = directoryName.trim();

    if (!name) {
      directoryError = "Directory name cannot be empty";
      nameInput?.focus();
      return;
    }

    const rootId = root.id;
    const parent = location.id;

    directorySaving = true;
    directoryError = "";

    try {
      const created = await session.client.createDirectory(
        rootId,
        parent,
        name,
      );

      // Prevent inserting the entry if the view has changed
      if (root.id !== rootId || location.id !== parent) {
        return;
      }

      insertEntry(created);

      drafting = false;
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

  function toggleAll() {
    if (selected.length === entries.length) {
      selected = [];
      return;
    }

    selected = entries.map((entry) => entry.file.id);
  }

  function askDelete(ids: string[]) {
    pendingDelete = ids.slice();
    deleteOpen = true;
  }

  const deleteDescription = $derived.by(() => {
    if (pendingDelete.length === 1) {
      const entry = entries.find((item) => item.file.id === pendingDelete[0]);
      return `Delete ${entry ? label(entry) : "this item"}?`;
    }
    return `Delete ${pendingDelete.length} items?`;
  });

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
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  <div
    class="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-3 py-2"
  >
    <div class="min-w-0">
      <p class="inline truncate text-sm font-medium">{root.name}</p>
      <p class="inline truncate text-xs text-muted-foreground">
        · {root.owner ? "Private" : "Shared"}
        · {formatQuota(root.quota)}
        · Updated {formatDatetime(root.updatedAt)}
      </p>
    </div>
    {#if admin}
      <a
        href="/admin/folders/{root.owner ? 'private' : 'shared'}/{root.id}"
        class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Folder settings"
      >
        <Settings size={16} />
      </a>
    {/if}
  </div>

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

    <div class="flex shrink-0 items-center gap-2">
      <input
        bind:this={fileInput}
        type="file"
        class="sr-only"
        multiple
        tabindex={-1}
        aria-hidden="true"
        onchange={upload}
      />
      <Button
        type="button"
        variant="outline"
        aria-label="Upload"
        onclick={() => fileInput?.click()}
      >
        <Upload size={16} />
        <span class="hidden sm:inline">Upload</span>
      </Button>
      <Button type="button" aria-label="New folder" onclick={beginDirectory}>
        <FolderPlus size={16} />
        <span class="hidden sm:inline">New folder</span>
      </Button>
      {#if selected.length > 0}
        <Button
          type="button"
          variant="outline"
          aria-label="Delete selected"
          onclick={() => askDelete(selected)}
        >
          <Trash size={16} />
          <span class="hidden sm:inline">Delete</span>
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

    <div
      class="{columns} h-10 shrink-0 border-b text-xs font-medium text-muted-foreground"
    >
      <input
        type="checkbox"
        class="size-4"
        aria-label="Select all"
        checked={entries.length > 0 && selected.length === entries.length}
        disabled={entries.length === 0}
        onchange={toggleAll}
      />
      {@render columnHeader("name", "Name", "", "")}
      {@render columnHeader("size", "Size", "hidden @lg:flex", "justify-end")}
      {@render columnHeader("created", "Created", "hidden @4xl:flex", "")}
      {@render columnHeader("modified", "Modified", "hidden @2xl:flex", "")}
      <span class="sr-only">Actions</span>
    </div>

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
      {:else if entries.length === 0 && !drafting}
        <div
          class="flex h-full flex-col items-center justify-center gap-2 px-6 py-16 text-center"
        >
          <Folder class="size-10 text-muted-foreground opacity-40" />
          <p class="text-sm font-medium">This folder is empty</p>
          <p class="max-w-sm text-sm text-muted-foreground">
            Upload files or create a folder to get started.
          </p>
        </div>
      {:else}
        <ul class="divide-y">
          <!-- NEW DIRECTORY DRAFT -->

          {#if drafting}
            <li class="px-3 py-2">
              <div class="flex min-w-0 items-center gap-2">
                <Folder size={18} class="shrink-0 text-primary" />
                <input
                  use:focusDraft
                  bind:value={directoryName}
                  class={cn(
                    "h-8 min-w-0 flex-1 rounded-md border border-input bg-field px-2.5 text-sm shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50",
                    directoryError &&
                      "border-destructive focus-visible:ring-destructive",
                  )}
                  aria-label="Folder name"
                  aria-invalid={directoryError ? true : undefined}
                  autocomplete="off"
                  spellcheck="false"
                  placeholder="Folder name"
                  disabled={directorySaving}
                  oninput={() => (directoryError = "")}
                  onkeydown={onDraftKey}
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="size-8 px-0"
                  aria-label="Create folder"
                  disabled={directorySaving}
                  onclick={() => commitDirectory()}
                >
                  <Check size={16} />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="size-8 px-0"
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
            <FileListItem
              {entry}
              rootId={root.id}
              name={label(entry)}
              {columns}
              selected={selected.includes(entry.file.id)}
              onToggle={() => toggle(entry.file.id)}
              onOpen={() => openDirectory(entry)}
              onPreview={() => (previewFileId = entry.file.id)}
              onDownload={() => download(entry)}
              onDelete={() => askDelete([entry.file.id])}
            />
          {/each}
        </ul>
      {/if}
    </div>
  </div>

  {#if previewFileId !== null && imageEntries.some((entry) => entry.file.id === previewFileId)}
    <ImagePreview
      rootId={root.id}
      files={imageEntries}
      fileId={previewFileId}
      onClose={() => (previewFileId = null)}
      onSelect={(id) => (previewFileId = id)}
    />
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

{#snippet columnHeader(
  column: SortColumn,
  label: string,
  cellClass: string,
  buttonClass: string,
)}
  <div
    role="columnheader"
    aria-sort={ariaSort(column)}
    class={cn("flex h-full min-w-0 items-center", cellClass)}
  >
    <button
      type="button"
      class={cn(
        "inline-flex h-full w-full min-w-0 items-center gap-1 hover:text-foreground",
        sort.column === column && "text-foreground",
        buttonClass,
      )}
      onclick={() => cycleSort(column)}
    >
      <span class="truncate">{label}</span>
      {#if sort.column === column}
        {#if sort.direction === "asc"}
          <ArrowUp size={14} class="shrink-0" aria-hidden="true" />
        {:else}
          <ArrowDown size={14} class="shrink-0" aria-hidden="true" />
        {/if}
      {/if}
    </button>
  </div>
{/snippet}
