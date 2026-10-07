<script lang="ts">
  import { ApiError, type NimbusClient } from "../api";
  import type { FileEntry, Root } from "../types";
  import Button from "./ui/button/button.svelte";
  import Input from "./ui/input/input.svelte";

  let {
    client,
    folder,
  }: {
    client: NimbusClient;
    folder: Root;
  } = $props();

  interface PendingUpload {
    id: number;
    name: string;
    loaded: number;
    total: number;
    error: string;
  }

  /// How many files are sent at the same time.
  const PARALLEL_UPLOADS = 10;

  interface Location {
    id: string | null;
    name: string;
  }

  let entries = $state<FileEntry[]>([]);
  let directoryName = $state("");
  let error = $state("");
  let current = $state<Location | undefined>();
  let pending = $state<PendingUpload[]>([]);
  let nextUploadId = 0;

  $effect(() => {
    current = { id: null, name: folder.name };
    void load(null);
  });

  async function load(parent: string | null) {
    error = "";
    try {
      entries = await client.list(folder.id, parent);
    } catch (cause) {
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
    if (!current || files.length === 0) return;
    error = "";

    const parent = current.id;
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
          await uploadOne(parent, file, item);
        }
      },
    );
    await Promise.all(workers);

    if (current?.id === parent) await load(parent);
  }

  async function uploadOne(
    parent: string | null,
    file: File,
    item: PendingUpload,
  ) {
    try {
      await client.upload(folder.id, parent, file, (loaded, total) => {
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
    if (!current || !directoryName.trim()) return;
    error = "";
    try {
      await client.createDirectory(folder.id, current.id, directoryName.trim());
      directoryName = "";
      await load(current.id);
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? "Could not create the folder"
          : "The server could not be reached";
    }
  }

  async function remove(entry: FileEntry) {
    if (!current) return;
    error = "";
    try {
      await client.remove(folder.id, entry.id);
      await load(current.id);
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
    current = { id: entry.id, name: entry.name };
    void load(entry.id);
  }
</script>

{#if current}
  <div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 class="text-lg font-medium">{current.name}</h2>
      </div>
      <label class="text-sm">
        <span class="sr-only">Upload</span>
        <input type="file" multiple onchange={upload} />
      </label>
    </div>

    <form class="flex gap-2" onsubmit={mkdir}>
      <Input bind:value={directoryName} placeholder="New folder" />
      <Button type="submit" variant="outline">Create</Button>
    </form>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}

    {#if pending.length > 0}
      <ul class="space-y-2 rounded-lg border bg-card p-3">
        {#each pending as item (item.id)}
          <li class="space-y-1">
            <div class="flex items-center justify-between gap-3 text-sm">
              <span class="truncate">{item.name}</span>
              {#if item.error}
                <span class="flex items-center gap-2 text-destructive">
                  {item.error}
                  <button
                    class="text-xs underline"
                    onclick={() => dismiss(item)}>Dismiss</button
                  >
                </span>
              {:else}
                <span class="text-muted-foreground tabular-nums"
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

    {#if current.id}
      <Button
        variant="ghost"
        size="sm"
        onclick={() => {
          current = { id: null, name: folder.name };
          void load(null);
        }}
      >
        Back to root
      </Button>
    {/if}

    {#if entries.length === 0}
      <p class="text-sm text-muted-foreground">This folder is empty.</p>
    {:else}
      <ul class="divide-y rounded-lg border">
        {#each entries as entry (entry.id)}
          <li class="flex items-center justify-between gap-3 px-3 py-2">
            {#if entry.kind === "directory"}
              <button
                class="text-left text-sm font-medium"
                onclick={() => openDirectory(entry)}
              >
                {entry.name}
              </button>
            {:else}
              <button
                class="text-left text-sm hover:underline"
                onclick={() => download(entry)}
              >
                {entry.name}
              </button>
            {/if}
            <Button variant="ghost" size="sm" onclick={() => remove(entry)}
              >Delete</Button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}
