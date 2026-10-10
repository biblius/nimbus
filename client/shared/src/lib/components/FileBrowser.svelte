<script lang="ts">
  import { ExternalLink } from "@lucide/svelte";
  import { ApiError, ValidationError, type NimbusClient } from "../api";
  import { formatQuota, formatDatetime } from "../format";
  import { toastError } from "../toast";
  import type { FileEntry, Root } from "../types";
  import Button from "./ui/button/button.svelte";
  import Field from "./ui/field/field.svelte";

  let {
    client,
    folder,
    admin = false,
  }: {
    client: NimbusClient;
    folder: Root;
    admin?: boolean;
  } = $props();

  type PendingUpload = {
    id: number;
    name: string;
    loaded: number;
    total: number;
    error: string;
  };

  type Crumb = {
    id: string | null;
    name: string;
  };

  /// How many files are sent at the same time.
  const PARALLEL_UPLOADS = 10;

  let entries = $state<FileEntry[]>([]);
  let directoryName = $state("");
  let directoryError = $state<ValidationError | undefined>();
  let pending = $state<PendingUpload[]>([]);
  let nextUploadId = 0;
  let ticket = 0;
  let descendants = $state<Crumb[]>([]);

  const trail = $derived.by(() => {
    const crumbs: Crumb[] = [{ id: null, name: folder.name }];
    for (const crumb of descendants) crumbs.push(crumb);
    return crumbs;
  });
  const here = $derived(trail[trail.length - 1]);

  $effect(() => {
    const rootId = folder.id;
    descendants = [];
    void load(rootId, null);
  });

  async function load(rootId: string, parent: string | null) {
    const currentTicket = ++ticket;
    try {
      const next = await client.list(rootId, parent);
      if (currentTicket !== ticket) {
        return;
      }
      entries = next;
    } catch (cause) {
      if (currentTicket !== ticket) {
        return;
      }
      entries = [];
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not list this folder",
      );
    }
  }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files: File[] = [];
    if (input.files) {
      for (const file of input.files) files.push(file);
    }
    input.value = "";
    if (files.length === 0) {
      return;
    }

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
          try {
            const created = await client.upload(
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
            if (folder.id === rootId && here.id === parent) {
              entries.push(created);
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

  async function mkdir(event: SubmitEvent) {
    event.preventDefault();
    directoryError = undefined;
    const rootId = folder.id;
    const parent = here.id;
    try {
      const created = await client.createDirectory(
        rootId,
        parent,
        directoryName.trim(),
      );
      directoryName = "";
      if (folder.id === rootId && here.id === parent) {
        entries.push(created);
      }
    } catch (cause) {
      if (cause instanceof ValidationError) {
        directoryError = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not create the folder",
      );
    }
  }

  async function remove(entry: FileEntry) {
    const rootId = folder.id;
    const parent = here.id;
    try {
      await client.remove(rootId, entry.id);
      if (folder.id === rootId && here.id === parent) {
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
      await client.download(folder.id, entry.id, entry.name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not download that file",
      );
    }
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
    class="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-3 py-2"
  >
    <div class="min-w-0">
      <p class="truncate text-sm font-medium">{folder.name}</p>
      <p class="text-xs text-muted-foreground">
        {folder.owner ? "Private" : "Shared"}
        · {formatQuota(folder.quota)}
        · Updated {formatDatetime(folder.updatedAt)}
      </p>
    </div>
    {#if admin}
      <a href="/admin/folders/{folder.owner ? 'private' : 'shared'}/{folder.id}"
        ><ExternalLink size={16} /></a
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
          onclick={() => {
            descendants = descendants.slice(0, index);
            void load(folder.id, crumb.id);
          }}
        >
          {crumb.name}
        </button>
      {/if}
    {/each}
  </nav>

  <div
    class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
  >
    <form class="flex min-w-0 flex-1 items-end gap-2" onsubmit={mkdir}>
      <div class="min-w-0 flex-1">
        <Field
          label="New folder"
          id="directory-name"
          bind:value={directoryName}
          error={directoryError?.field("name")}
        />
      </div>
      <Button type="submit" variant="outline">Create</Button>
    </form>
    <label class="text-sm">
      <span class="sr-only">Upload</span>
      <input type="file" multiple onchange={upload} />
    </label>
  </div>

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
              onclick={() => {
                descendants.push({ id: entry.id, name: entry.name });
                void load(folder.id, entry.id);
              }}
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
