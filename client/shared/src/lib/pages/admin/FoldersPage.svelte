<script lang="ts">
  import { untrack } from "svelte";
  import FolderListItem from "../../components/folder/FolderListItem.svelte";
  import RootForm from "../../components/folder/RootForm.svelte";
  import Button from "../../components/ui/button/button.svelte";
  import Plus from "../../components/ui/plus/plus.svelte";
  import { route } from "../../router";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData } from "../../types";

  let folders = $state<Root[]>([]);
  let shared = $state(false);
  let creating = $state(false);

  function reset() {
    creating = false;
  }

  async function load() {
    const isShared = shared;
    const roots = isShared
      ? await session.client.listSharedFolders()
      : await session.client.listPrivateFolders();

    if (isShared !== shared) return;

    folders = roots;
  }

  $effect(() => {
    const isShared = route.pathname === "/admin/folders/shared";
    untrack(() => {
      shared = isShared;
      reset();
      load().catch((cause) => {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load folders",
        );
      });
    });
  });

  async function createRoot(data: RootData) {
    const created = await session.client.createFolder(data);

    folders = [created, ...folders];

    reset();
  }

  function updateFolder(updated: Root) {
    folders = [
      updated,
      ...folders.filter((folder) => folder.id !== updated.id),
    ];
  }

  function removeFolder(removed: Root) {
    folders = folders.filter((folder) => folder.id !== removed.id);
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
      <RootForm
        title="Add folder"
        description="A quota of 0 means unlimited. Choose who can use this shared folder."
        save={createRoot}
        cancel={reset}
      />
    </div>
  {/if}

  {#if folders.length === 0}
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
