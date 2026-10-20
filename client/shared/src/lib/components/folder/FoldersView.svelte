<script lang="ts">
  import { Folder } from "@lucide/svelte";
  import { onMount } from "svelte";
  import FolderListItem from "./FolderListItem.svelte";
  import SharedRootForm from "./SharedRootForm.svelte";
  import Button from "../ui/button/button.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData } from "../../types";

  let {
    shared = false,
    load,
  }: {
    shared?: boolean;
    load: () => Promise<Root[]>;
  } = $props();

  let folders = $state<Root[] | null>(null);
  let failed = $state(false);
  let creating = $state(false);

  function reset() {
    creating = false;
  }

  onMount(() => {
    load()
      .then((roots) => {
        folders = roots;
      })
      .catch((cause) => {
        failed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load folders",
        );
      });
  });

  async function createRoot(data: RootData) {
    const created = await session.client.createFolder(data);

    if (!folders) {
      folders = [];
    }
    folders.unshift(created);

    reset();
  }

  function updateFolder(updated: Root) {
    if (!folders) {
      folders = [];
    }

    const index = folders.findIndex((folder) => folder.id === updated.id);
    if (index !== -1) {
      folders.splice(index, 1);
    }
    folders.unshift(updated);
  }

  function removeFolder(removed: Root) {
    if (!folders) {
      return;
    }

    const index = folders.findIndex((folder) => folder.id === removed.id);
    if (index !== -1) {
      folders.splice(index, 1);
    }
  }
</script>

<div class="min-h-0 flex-1 space-y-5 overflow-y-auto overscroll-contain">
  <div class="flex flex-wrap items-end justify-between gap-3 border-b pb-4">
    <div class="min-w-0 space-y-1">
      <h2 class="text-lg font-semibold tracking-tight">
        {shared ? "Shared folders" : "Private folders"}
      </h2>
      <p class="text-sm text-muted-foreground">
        {#if shared}
          Team roots with quotas and membership.
        {:else}
          Each user’s personal root and storage quota.
        {/if}
      </p>
    </div>
    {#if shared}
      <Button
        type="button"
        variant={creating ? "outline" : "default"}
        size="sm"
        aria-expanded={creating}
        onclick={() => (creating = !creating)}
      >
        {creating ? "Cancel" : "Add folder"}
      </Button>
    {/if}
  </div>

  {#if creating}
    <div class="mx-auto w-full max-w-2xl">
      <SharedRootForm
        title="Add folder"
        description="Set a storage cap and choose who can use this folder."
        save={createRoot}
        cancel={reset}
      />
    </div>
  {/if}

  {#if failed}
    <div
      class="rounded-xl border border-dashed px-6 py-12 text-center text-sm text-muted-foreground"
    >
      Could not load folders.
    </div>
  {:else if !folders}
    <div
      class="rounded-xl border border-dashed px-6 py-12 text-center text-sm text-muted-foreground"
    >
      Loading…
    </div>
  {:else if folders.length === 0}
    <div
      class="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed px-6 py-16 text-center"
    >
      <Folder class="size-9 text-muted-foreground opacity-50" />
      <p class="text-sm font-medium">
        {shared ? "No shared folders yet" : "No private folders yet"}
      </p>
      <p class="max-w-sm text-sm text-muted-foreground">
        {#if shared}
          Create a shared folder to give people a common workspace.
        {:else}
          Private folders appear when users are created.
        {/if}
      </p>
    </div>
  {:else}
    <ul class="overflow-hidden rounded-xl border bg-card">
      {#each folders as folder, index (folder.id)}
        <FolderListItem
          {folder}
          bordered={index > 0}
          onUpdate={updateFolder}
          onDelete={removeFolder}
        />
      {/each}
    </ul>
  {/if}
</div>
