<script lang="ts">
  import { Pencil, Trash } from "@lucide/svelte";
  import MemberInfo from "./MemberInfo.svelte";
  import PrivateRootForm from "./PrivateRootForm.svelte";
  import SharedRootForm from "./SharedRootForm.svelte";
  import Button from "../ui/button/button.svelte";
  import Chevron from "../ui/chevron/chevron.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import { formatBytes, formatDatetime, formatQuota } from "../../format";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData } from "../../types";

  let {
    folder,
    onUpdate,
    onDelete,
  }: {
    folder: Root;
    onUpdate: (folder: Root) => void;
    onDelete: (folder: Root) => void;
  } = $props();

  let expanded = $state(false);
  let editing = $state(false);
  let deleteOpen = $state(false);
  let deleting = $state(false);
  /**
   * Bytes used.
   * Loads on initial component load.
   */
  let usage = $state<number | undefined>();

  const shared = $derived(folder.owner == null);

  function summary() {
    const quota = formatQuota(folder.quota);

    if (usage === undefined) {
      return quota;
    }

    return `${formatBytes(usage)} / ${quota}`;
  }

  function loadUsage() {
    const id = folder.id;

    usage = undefined;

    session.client
      .folderUsage(id)
      .then((bytes) => {
        usage = bytes;
      })
      .catch(() => {
        usage = undefined;
      });
  }

  function toggle() {
    if (expanded) {
      expanded = false;
      editing = false;
      return;
    }

    expanded = true;

    loadUsage();
  }

  async function save(data: RootData) {
    const updated = await session.client.updateFolder(folder.id, data);

    editing = false;

    onUpdate(updated);
  }

  async function confirmDelete() {
    if (deleting) {
      return;
    }

    deleting = true;

    try {
      await session.client.deleteFolder(folder.id);
      onDelete(folder);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not remove the folder",
      );
      deleting = false;
    }
  }
</script>

<li>
  <button
    type="button"
    class="flex min-h-11 w-full items-center justify-between gap-3 px-3 py-2 text-left {expanded
      ? 'bg-accent'
      : 'hover:bg-accent/60'}"
    aria-expanded={expanded}
    onclick={toggle}
  >
    <span class="min-w-0">
      <span class="block truncate text-sm font-medium">{folder.name}</span>
      <span class="block truncate text-xs text-muted-foreground">
        {#if folder.owner}
          <span class="font-mono">{folder.owner}</span> ·
        {/if}
        {summary()}
      </span>
    </span>

    <Chevron {expanded} />
  </button>

  {#if expanded}
    <div class="border-t bg-muted/30 p-3">
      {#if editing}
        <div class="mx-auto max-w-lg">
          {#if shared}
            <SharedRootForm
              {folder}
              title="Edit folder"
              description="Choose who can use this shared folder."
              {save}
              cancel={() => (editing = false)}
            />
          {:else}
            <PrivateRootForm
              {folder}
              title="Edit folder"
              description="A private folder stays with its owner."
              {save}
              cancel={() => (editing = false)}
            />
          {/if}
        </div>
      {:else}
        <div class="flex justify-between gap-3">
          <div class="min-w-0 flex-1 space-y-4">
            <div class="flex flex-wrap items-start justify-between gap-3">
              <dl
                class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm"
              >
                <dt class="text-muted-foreground">ID</dt>
                <dd class="truncate font-mono text-xs leading-5">
                  {folder.id}
                </dd>
                <dt class="text-muted-foreground">Name</dt>
                <dd class="truncate">{folder.name}</dd>
                {#if folder.owner}
                  <dt class="text-muted-foreground">Owner</dt>
                  <dd class="truncate font-mono text-xs leading-5">
                    {folder.owner}
                  </dd>
                {:else}
                  <dt class="text-muted-foreground">Type</dt>
                  <dd>Shared</dd>
                {/if}
                <dt class="text-muted-foreground">Quota</dt>
                <dd>{formatQuota(folder.quota)}</dd>
                <dt class="text-muted-foreground">Usage</dt>
                <dd>{usage === undefined ? "-" : formatBytes(usage)}</dd>
                <dt class="text-muted-foreground">Created</dt>
                <dd>{formatDatetime(folder.createdAt)}</dd>
                <dt class="text-muted-foreground">Updated</dt>
                <dd>{formatDatetime(folder.updatedAt)}</dd>
              </dl>
            </div>

            {#if shared}
              <MemberInfo folderId={folder.id} />
            {/if}
          </div>
          <div class="flex flex-col self-stretch">
            <Button
              type="button"
              variant="outline"
              size="sm"
              class="size-8 p-0"
              aria-label="Edit folder"
              title="Edit folder"
              onclick={() => (editing = true)}
            >
              <Pencil size={16} />
            </Button>
            {#if shared}
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="mt-auto size-8 p-0"
                aria-label="Delete folder"
                title="Delete folder"
                onclick={() => (deleteOpen = true)}
              >
                <Trash size={16} />
              </Button>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <ConfirmDialog
    open={deleteOpen}
    title="Delete folder"
    description={`Delete ${folder.name}? This removes the folder and everything in it.`}
    busy={deleting}
    onCancel={() => {
      if (!deleting) {
        deleteOpen = false;
      }
    }}
    onConfirm={confirmDelete}
  />
</li>
