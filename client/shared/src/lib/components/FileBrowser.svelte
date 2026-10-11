<script lang="ts">
  import {
    Check,
    ChevronRight,
    File as FileIcon,
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
  import { formatBytes, formatQuota, formatDatetime } from "../format";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { FileEntry, Root } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";

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

  const columns =
    "grid grid-cols-[minmax(0,1fr)_2.25rem] items-center gap-x-3 px-3 sm:grid-cols-[minmax(0,1fr)_6rem_11.5rem_2.25rem] sm:gap-x-4";

  let entries = $state<FileEntry[]>([]);

  let loading = $state(true);
  let latestLoad = 0;

  let drafting = $state(false);

  let directoryName = $state("");
  let directoryError = $state("");
  let directorySaving = $state(false);

  let pending = $state<PendingUpload[]>([]);

  let nextUploadId = 0;

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

    loadAncestors(rootId, parent, currentLoad);

    try {
      const next = await session.client.list(rootId, parent);

      if (currentLoad !== latestLoad) {
        return;
      }

      entries = next.filter((entry) => entry.parent === parent);
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

  /// Drop the returned entry into the open folder. Directories stay ahead of files, then by name.
  function insertEntry(created: FileEntry) {
    // Do not add files if the location changed
    if (created.parent !== location.id) {
      return;
    }

    if (entries.length === 0) {
      entries.push(created);
      return;
    }

    const index = entries.findIndex((entry) => {
      if (created.kind !== entry.kind) {
        return created.kind === "directory";
      }
      return created.name < entry.name;
    });

    if (index === -1) {
      entries.push(created);
    } else {
      entries.splice(index, 0, created);
    }
  }

  function openDirectory(entry: FileEntry) {
    ancestors = [...ancestors, { id: entry.id, name: entry.name }];
    showDirectory(entry.id);
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
      pending.push({
        id: nextUploadId++,
        name: file.name,
        loaded: 0,
        total: file.size,
        error: "",
      });

      // Read the element back so later writes go through the reactive proxy.
      const item = pending[pending.length - 1];

      return { file, item };
    });

    const workers = Array.from(
      { length: Math.min(PARALLEL_UPLOADS, queue.length) },
      async () => {
        while (queue.length > 0) {
          const { file, item } = queue.shift()!;

          try {
            const created = await session.client.upload(
              rootId,
              parent,
              file,
              (loaded, total) => {
                item.loaded = loaded;
                item.total = total;
              },
            );

            const index = pending.findIndex((entry) => entry.id === item.id);

            if (index !== -1) {
              pending.splice(index, 1);
            }

            if (root.id === rootId && location.id === parent) {
              insertEntry(created);
            }
          } catch (cause) {
            if (cause instanceof TypeError) {
              item.error = "The server could not be reached";
            } else if (cause instanceof ApiError && cause.status === 409) {
              item.error = "A file with this name already exists";
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
    if (cause instanceof ApiError && cause.status === 409) {
      return "A folder with this name already exists";
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

  async function remove(entry: FileEntry) {
    const currentRoot = root.id;
    const currentParent = location.id;

    try {
      await session.client.remove(currentRoot, entry.id);

      if (root.id === currentRoot && location.id === currentParent) {
        entries = entries.filter((item) => item.id !== entry.id);
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not delete that item",
      );
    }
  }

  async function download(entry: FileEntry) {
    try {
      await session.client.download(root.id, entry.id, entry.name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not download that file",
      );
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3">
  <div
    class="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-3 py-2"
  >
    <div class="min-w-0">
      <p class="truncate text-sm font-medium">{root.name}</p>
      <p class="text-xs text-muted-foreground">
        {root.owner ? "Private" : "Shared"}
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
        onclick={() => fileInput?.click()}
      >
        <Upload size={16} />
        Upload
      </Button>
      <Button type="button" onclick={beginDirectory}>
        <FolderPlus size={16} />
        New folder
      </Button>
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
              <span class="shrink-0 text-muted-foreground tabular-nums"
                >{percent(item)}%</span
              >
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
    class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card"
  >
    <div
      class="{columns} h-10 shrink-0 border-b text-xs font-medium text-muted-foreground"
    >
      <span>Name</span>
      <span class="hidden text-right sm:block">Size</span>
      <span class="hidden sm:block">Modified</span>
      <span class="sr-only">Actions</span>
    </div>

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
          <!-- CREATE DIRECTORY -->

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

          {#each entries as entry (entry.id)}
            <li class="{columns} group min-h-11 py-1 hover:bg-accent/60">
              {#if entry.kind === "directory"}
                <button
                  type="button"
                  class="flex min-h-11 w-full min-w-0 items-center gap-2.5 text-left text-sm font-medium"
                  onclick={() => openDirectory(entry)}
                >
                  <Folder size={18} class="shrink-0 text-primary" />
                  <span class="truncate" title={entry.name}>{entry.name}</span>
                </button>
              {:else}
                <button
                  type="button"
                  class="flex min-h-11 w-full min-w-0 items-center gap-2.5 text-left text-sm hover:underline"
                  onclick={() => download(entry)}
                >
                  <FileIcon size={18} class="shrink-0 text-muted-foreground" />
                  <span class="truncate" title={entry.name}>{entry.name}</span>
                </button>
              {/if}
              <span
                class="hidden text-right text-sm text-muted-foreground tabular-nums sm:block"
              >
                {entry.kind === "directory"
                  ? "—"
                  : formatBytes(entry.sizeBytes)}
              </span>
              <span
                class="hidden min-w-0 truncate text-sm text-muted-foreground sm:block"
              >
                {formatDatetime(entry.updatedAt)}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="size-8 px-0 text-muted-foreground hover:text-destructive sm:opacity-0 sm:group-focus-within:opacity-100 sm:group-hover:opacity-100"
                aria-label="Delete {entry.name}"
                onclick={() => remove(entry)}
              >
                <Trash size={16} />
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
