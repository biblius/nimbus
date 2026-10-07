<script lang="ts">
  import { ApiError, type NimbusClient } from "../api";
  import { formatQuota, formatWhen } from "../format";
  import type { FileEntry, Root } from "../types";
  import Button from "./ui/button/button.svelte";
  import Input from "./ui/input/input.svelte";

  let {
    client,
    folder,
    admin = false,
    onEditRoot,
  }: {
    client: NimbusClient;
    folder: Root;
    admin?: boolean;
    onEditRoot?: () => void;
  } = $props();

  interface PendingUpload {
    id: number;
    name: string;
    loaded: number;
    total: number;
    error: string;
  }

  interface Crumb {
    id: string | null;
    name: string;
  }

  /// How many files are sent at the same time.
  const PARALLEL_UPLOADS = 10;

  let entries = $state<FileEntry[]>([]);
  let directoryName = $state("");
  let error = $state("");
  let pending = $state<PendingUpload[]>([]);
  let nextUploadId = 0;
  let ticket = 0;
  let descendants = $state<Crumb[]>([]);

  const trail = $derived([
    { id: null as string | null, name: folder.name },
    ...descendants,
  ]);
  const here = $derived(trail[trail.length - 1]);

  $effect(() => {
    const rootId = folder.id;
    descendants = [];
    void load(rootId, null);
  });

  async function load(rootId: string, parent: string | null) {
    const currentTicket = ++ticket;
    error = "";
    try {
      const next = await client.list(rootId, parent);
      if (currentTicket !== ticket) return;
      entries = next;
    } catch (cause) {
      if (currentTicket !== ticket) return;
      entries = [];
      error =
        cause instanceof ApiError
          ? "Could not list this folder"
          : "The server could not be reached";
    }
  }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = [...(input.files ?? [])];
    input.value = "";
    if (files.length === 0) return;
    error = "";

    const rootId = folder.id;
    const parent = here.id;
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
          await uploadOne(rootId, parent, file, item);
        }
      },
    );
    await Promise.all(workers);

    if (folder.id === rootId && here.id === parent) await load(rootId, parent);
  }

  async function uploadOne(
    rootId: string,
    parent: string | null,
    file: File,
    item: PendingUpload,
  ) {
    try {
      await client.upload(rootId, parent, file, (loaded, total) => {
        item.loaded = loaded;
        item.total = total;
      });
      pending = pending.filter((entry) => entry.id !== item.id);
    } catch (cause) {
      item.error =
        cause instanceof ApiError
          ? describeUploadError(cause)
          : "The server could not be reached";
    }
  }

  function describeUploadError(cause: ApiError) {
    switch (cause.status) {
      case 409:
        return "A file with this name already exists";
      case 413:
        return "Not enough space";
      case 403:
        return "You cannot upload here";
      default:
        return "Upload failed";
    }
  }

  function dismiss(item: PendingUpload) {
    pending = pending.filter((entry) => entry.id !== item.id);
  }

  function percent(item: PendingUpload) {
    if (item.total <= 0) return 0;
    return Math.min(100, Math.round((item.loaded / item.total) * 100));
  }

  async function mkdir(event: SubmitEvent) {
    event.preventDefault();
    if (!directoryName.trim()) return;
    error = "";
    const rootId = folder.id;
    const parent = here.id;
    try {
      await client.createDirectory(rootId, parent, directoryName.trim());
      directoryName = "";
      if (folder.id === rootId && here.id === parent)
        await load(rootId, parent);
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? "Could not create the folder"
          : "The server could not be reached";
    }
  }

  async function remove(entry: FileEntry) {
    error = "";
    const rootId = folder.id;
    const parent = here.id;
    try {
      await client.remove(rootId, entry.id);
      if (folder.id === rootId && here.id === parent)
        await load(rootId, parent);
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? "Could not delete that item"
          : "The server could not be reached";
    }
  }

  async function download(entry: FileEntry) {
    error = "";
    try {
      await client.download(folder.id, entry.id, entry.name);
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? "Could not download that file"
          : "The server could not be reached";
    }
  }

  function openDirectory(entry: FileEntry) {
    descendants = [...descendants, { id: entry.id, name: entry.name }];
    void load(folder.id, entry.id);
  }

  function go(index: number) {
    const crumb = trail[index];
    if (!crumb || index === trail.length - 1) return;
    descendants = descendants.slice(0, index);
    void load(folder.id, crumb.id);
  }
</script>

{#snippet entryIcon(directory: boolean)}
  <svg
    viewBox="0 0 20 20"
    class="size-4 shrink-0 {directory
      ? 'text-primary'
      : 'text-muted-foreground'}"
    fill="none"
    stroke="currentColor"
    aria-hidden="true"
  >
    {#if directory}
      <path
        d="M3.25 6.75A1.75 1.75 0 0 1 5 5h3.05c.3 0 .6.12.8.34l1.12 1.16H15a1.75 1.75 0 0 1 1.75 1.75v6A1.75 1.75 0 0 1 15 16H5a1.75 1.75 0 0 1-1.75-1.75v-7.5Z"
        stroke-width="1.5"
        stroke-linejoin="round"
      />
    {:else}
      <path
        d="M6.25 3.25h4.4c.2 0 .39.08.53.22l3.35 3.35c.14.14.22.33.22.53v8.4a1 1 0 0 1-1 1h-7.5a1 1 0 0 1-1-1v-11.5a1 1 0 0 1 1-1Z"
        stroke-width="1.5"
        stroke-linejoin="round"
      />
      <path d="M10.5 3.5V7h3.5" stroke-width="1.5" stroke-linejoin="round" />
    {/if}
  </svg>
{/snippet}

<div class="space-y-4">
  <div
    class="flex flex-wrap items-start justify-between gap-3 rounded-lg border bg-card px-3 py-2"
  >
    <div class="min-w-0">
      <p class="truncate text-sm font-medium">{folder.name}</p>
      <p class="text-xs text-muted-foreground">
        {folder.primary ? "Private" : "Shared"}
        · {formatQuota(folder.quota)}
        · Updated {formatWhen(folder.updatedAt)}
      </p>
    </div>
    {#if admin && onEditRoot}
      <Button type="button" variant="outline" size="sm" onclick={onEditRoot}
        >Edit root</Button
      >
    {/if}
  </div>

  <nav
    class="flex flex-wrap items-center gap-1 text-sm"
    aria-label="Current folder"
  >
    {#each trail as crumb, index (crumb.id ?? "root")}
      {#if index > 0}
        <span class="text-muted-foreground" aria-hidden="true">/</span>
      {/if}
      {#if index === trail.length - 1}
        <span
          class="max-w-40 truncate font-medium sm:max-w-none"
          aria-current="page"
        >
          {crumb.name}
        </span>
      {:else}
        <button
          type="button"
          class="max-w-40 truncate rounded-md px-1.5 py-1 text-muted-foreground hover:bg-accent hover:text-foreground hover:underline sm:max-w-none"
          onclick={() => go(index)}
        >
          {crumb.name}
        </button>
      {/if}
    {/each}
  </nav>

  <div
    class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
  >
    <form class="flex min-w-0 flex-1 gap-2" onsubmit={mkdir}>
      <Input bind:value={directoryName} placeholder="New folder" />
      <Button type="submit" variant="outline">Create</Button>
    </form>
    <label class="text-sm">
      <span class="sr-only">Upload</span>
      <input type="file" multiple onchange={upload} />
    </label>
  </div>

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {/if}

  {#if pending.length > 0}
    <ul class="space-y-2 rounded-lg border bg-card p-3">
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
                  onclick={() => dismiss(item)}
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

  {#if entries.length === 0}
    <p class="text-sm text-muted-foreground">This folder is empty.</p>
  {:else}
    <ul class="divide-y rounded-lg border">
      {#each entries as entry (entry.id)}
        <li class="flex items-center justify-between gap-3 px-3 py-1">
          {#if entry.kind === "directory"}
            <button
              type="button"
              class="flex min-h-11 min-w-0 flex-1 items-center gap-2 text-left text-sm font-medium"
              onclick={() => openDirectory(entry)}
            >
              {@render entryIcon(true)}
              <span class="truncate">{entry.name}</span>
            </button>
          {:else}
            <button
              type="button"
              class="flex min-h-11 min-w-0 flex-1 items-center gap-2 text-left text-sm hover:underline"
              onclick={() => download(entry)}
            >
              {@render entryIcon(false)}
              <span class="truncate">{entry.name}</span>
            </button>
          {/if}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onclick={() => remove(entry)}
          >
            Delete
          </Button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
