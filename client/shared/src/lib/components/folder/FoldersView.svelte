<script lang="ts">
  import { onMount } from "svelte";
  import FolderListItem from "./FolderListItem.svelte";
  import SharedRootForm from "./SharedRootForm.svelte";
  import Button from "../ui/button/button.svelte";
  import Plus from "../ui/plus/plus.svelte";
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

<div class="space-y-3">
  <div class="flex h-12 items-center justify-between gap-3 border-b px-1">
    <h2 class="text-sm font-medium">
      {shared ? "Shared folders" : "Private folders"}
    </h2>
    {#if shared}
      <Button
        type="button"
        variant={creating ? "outline" : "ghost"}
        size="sm"
        class="size-9 p-0"
        aria-label={creating ? "Close new folder form" : "Add folder"}
        aria-expanded={creating}
        title="Add folder"
        onclick={() => (creating = !creating)}
      >
        <Plus bind:rotated={creating} />
      </Button>
    {/if}
  </div>

  {#if creating}
    <div class="mx-auto max-w-lg">
      <SharedRootForm
        title="Add folder"
        description="A quota of 0 means unlimited. Choose who can use this shared folder."
        save={createRoot}
        cancel={reset}
      />
    </div>
  {/if}

  {#if failed}
    <p class="text-sm text-muted-foreground">Could not load folders.</p>
  {:else if !folders}
    <p class="text-sm text-muted-foreground">Loading…</p>
  {:else if folders.length === 0}
    <p class="text-sm text-muted-foreground">
      {shared ? "No shared folders yet." : "No private folders yet."}
    </p>
  {:else}
    <ul class="divide-y overflow-hidden rounded-lg border">
      {#each folders as folder (folder.id)}
        <FolderListItem
          {folder}
          onUpdate={updateFolder}
          onDelete={removeFolder}
        />
      {/each}
    </ul>
  {/if}
</div>
