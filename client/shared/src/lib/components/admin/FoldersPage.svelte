<script lang="ts">
  import { onMount } from "svelte";
  import { ApiError } from "../../api";
  import { formatQuota, formatWhen } from "../../format";
  import type { Folder } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Input from "../ui/input/input.svelte";
  import Label from "../ui/label/label.svelte";
  import type { AdminContext } from "./categories";

  let { context }: { context: AdminContext } = $props();

  let folders = $state<Folder[]>([]);
  let selectedId = $state<string | null>(null);
  let name = $state("");
  let quota = $state("0");
  let error = $state("");
  let notice = $state("");
  let busy = $state(false);
  let seen = $state(0);

  const selected = $derived(
    folders.find((folder) => folder.id === selectedId) ?? null,
  );
  const quotaPreview = $derived.by(() => {
    if (!/^\d+$/.test(quota.trim())) return "";
    return formatQuota(Number(quota));
  });

  function choose(folder: Folder) {
    selectedId = folder.id;
    name = folder.name;
    quota = String(folder.quota);
    error = "";
    notice = "";
  }

  async function refresh() {
    folders = await context.client.listFolders();
    const request = context.editRequest;
    const requested =
      request && request.token !== seen
        ? folders.find((folder) => folder.id === request.id)
        : undefined;
    if (requested && request) {
      choose(requested);
      seen = request.token;
      return;
    }
    const current = folders.find((folder) => folder.id === selectedId);
    if (current) {
      name = current.name;
      quota = String(current.quota);
      return;
    }
    if (folders[0]) choose(folders[0]);
    else selectedId = null;
  }

  onMount(() => {
    refresh().catch(() => {
      error = "Could not load folders";
    });
  });

  $effect(() => {
    const request = context.editRequest;
    if (!request || request.token === seen) return;
    const folder = folders.find((item) => item.id === request.id);
    if (!folder) return;
    choose(folder);
    seen = request.token;
  });

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const folder = selected;
    if (!folder) return;
    error = "";
    notice = "";
    if (!/^\d+$/.test(quota.trim())) {
      error = "Quota must be a whole number of bytes, or 0 for unlimited";
      return;
    }
    const quotaBytes = Number(quota);
    if (!Number.isSafeInteger(quotaBytes)) {
      error = "Quota must be a whole number of bytes, or 0 for unlimited";
      return;
    }

    busy = true;
    try {
      await context.client.updateFolder(folder.id, name.trim(), quotaBytes);
      notice = "Folder updated.";
      await refresh();
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? cause.status === 422
            ? "Check the folder name and quota"
            : "Could not update the folder"
          : "The server could not be reached";
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="grid items-start gap-6 lg:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]"
>
  <div class="space-y-3">
    <h2 class="text-lg font-medium">Folders</h2>
    {#if folders.length === 0}
      <p class="text-sm text-muted-foreground">No folders yet.</p>
    {:else}
      <ul class="divide-y overflow-hidden rounded-lg border">
        {#each folders as folder (folder.id)}
          <li>
            <button
              type="button"
              class="flex min-h-11 w-full flex-col items-start justify-center px-3 py-2 text-left {selectedId ===
              folder.id
                ? 'bg-accent'
                : 'hover:bg-accent/60'}"
              onclick={() => choose(folder)}
            >
              <span class="w-full truncate text-sm font-medium"
                >{folder.name}</span
              >
              <span class="text-xs text-muted-foreground"
                >{formatQuota(folder.quota)}</span
              >
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if selected}
    <form
      class="max-w-lg space-y-4 rounded-xl border bg-card p-4"
      onsubmit={save}
    >
      <div>
        <h3 class="text-base font-medium">Edit folder</h3>
        <p class="text-sm text-muted-foreground">
          Updated {formatWhen(selected.updatedAt)}. A quota of 0 means
          unlimited.
        </p>
      </div>

      <div class="space-y-2">
        <Label for="folder-name">Name</Label>
        <Input id="folder-name" bind:value={name} required />
      </div>

      <div class="space-y-2">
        <Label for="folder-quota">Quota (bytes)</Label>
        <Input
          id="folder-quota"
          inputmode="numeric"
          bind:value={quota}
          required
        />
        {#if quotaPreview}
          <p class="text-xs text-muted-foreground">{quotaPreview}</p>
        {/if}
      </div>

      {#if error}
        <p class="text-sm text-destructive">{error}</p>
      {/if}
      {#if notice}
        <p class="text-sm">{notice}</p>
      {/if}

      <Button type="submit" disabled={busy}>Save</Button>
    </form>
  {:else}
    <p class="text-sm text-muted-foreground">
      Select a folder to edit its name and quota.
    </p>
  {/if}
</div>
