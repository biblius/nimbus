<script lang="ts">
  import { Folder, Pencil, Trash } from "@lucide/svelte";
  import { onMount } from "svelte";
  import MemberInfo from "./MemberInfo.svelte";
  import PrivateRootForm from "./PrivateRootForm.svelte";
  import SharedRootForm from "./SharedRootForm.svelte";
  import Button from "../ui/button/button.svelte";
  import Chevron from "../ui/chevron/chevron.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import { trackDeletion } from "../../deletion.svelte";
  import { formatBytes, formatDatetime, formatQuota } from "../../format";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData } from "../../types";

  let {
    folder,
    bordered = false,
    onUpdate,
    onDelete,
  }: {
    folder: Root;
    bordered?: boolean;
    onUpdate: (folder: Root) => void;
    onDelete: (folder: Root) => void;
  } = $props();

  let expanded = $state(false);
  let editing = $state(false);
  let deleteOpen = $state(false);
  let deleting = $state(false);
  let usage = $state<number | undefined>();
  let usageGeneration = 0;

  const shared = $derived(folder.owner == null);

  const usageRatio = $derived.by(() => {
    if (folder.quota <= 0 || usage === undefined) {
      return null;
    }
    return Math.min(1, usage / folder.quota);
  });

  function summary() {
    const quota = formatQuota(folder.quota);

    if (usage === undefined) {
      return `… / ${quota}`;
    }

    return `${formatBytes(usage)} / ${quota}`;
  }

  function loadUsage() {
    const id = folder.id;
    const current = ++usageGeneration;

    session.client
      .folderUsage(id)
      .then((bytes) => {
        if (current === usageGeneration) {
          usage = bytes;
        }
      })
      .catch(() => {
        if (current === usageGeneration) {
          usage = undefined;
        }
      });
  }

  onMount(() => {
    loadUsage();
  });

  function toggle() {
    if (expanded) {
      expanded = false;
      editing = false;
      return;
    }

    expanded = true;
  }

  async function save(data: RootData) {
    const updated = await session.client.updateFolder(folder.id, data);

    editing = false;

    onUpdate(updated);
    loadUsage();
  }

  async function confirmDelete() {
    if (deleting) {
      return;
    }

    deleting = true;
    deleteOpen = false;

    try {
      await trackDeletion(session.client.deleteFolder(folder.id));
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

<li class={bordered ? "border-t" : ""}>
  <button
    type="button"
    class="flex min-h-14 w-full items-center gap-3 px-3 py-2.5 text-left transition-colors {expanded
      ? 'bg-accent'
      : 'hover:bg-accent/50'}"
    aria-expanded={expanded}
    onclick={toggle}
  >
    <span
      class="inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-secondary text-secondary-foreground"
      aria-hidden="true"
    >
      <Folder size={16} />
    </span>
    <span class="min-w-0 flex-1">
      <span class="flex min-w-0 items-center gap-2">
        <span class="truncate text-sm font-medium">{folder.name}</span>
        <span
          class="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-medium {shared
            ? 'bg-primary/15 text-primary'
            : 'bg-muted text-muted-foreground'}"
        >
          {shared ? "Shared" : "Private"}
        </span>
      </span>
      <span class="mt-0.5 block truncate text-xs text-muted-foreground">
        {summary()}
      </span>
      {#if usageRatio != null}
        <span
          class="mt-1.5 block h-1 max-w-48 overflow-hidden rounded-full bg-muted"
          aria-hidden="true"
        >
          <span
            class="block h-full rounded-full {usageRatio > 0.9
              ? 'bg-destructive'
              : 'bg-primary'}"
            style:width="{usageRatio * 100}%"
          ></span>
        </span>
      {/if}
    </span>
    <Chevron {expanded} />
  </button>

  {#if expanded}
    <div class="border-t bg-muted/25 px-3 py-4 sm:px-4">
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
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="min-w-0 flex-1 space-y-4">
            <dl
              class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-2 text-sm"
            >
              <dt class="text-muted-foreground">ID</dt>
              <dd class="truncate font-mono text-xs leading-5">{folder.id}</dd>
              <dt class="text-muted-foreground">Name</dt>
              <dd class="truncate">{folder.name}</dd>
              {#if folder.owner}
                <dt class="text-muted-foreground">Owner</dt>
                <dd class="truncate font-mono text-xs leading-5">
                  {folder.owner}
                </dd>
              {/if}
              <dt class="text-muted-foreground">Quota</dt>
              <dd>{formatQuota(folder.quota)}</dd>
              <dt class="text-muted-foreground">Usage</dt>
              <dd>{usage === undefined ? "…" : formatBytes(usage)}</dd>
              <dt class="text-muted-foreground">Created</dt>
              <dd>{formatDatetime(folder.createdAt)}</dd>
              <dt class="text-muted-foreground">Updated</dt>
              <dd>{formatDatetime(folder.updatedAt)}</dd>
            </dl>

            {#if shared}
              <MemberInfo folderId={folder.id} />
            {/if}
          </div>
          <div class="flex shrink-0 gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Edit folder"
              title="Edit folder"
              onclick={() => (editing = true)}
            >
              <Pencil size={15} />
              Edit
            </Button>
            {#if shared}
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                aria-label="Delete folder"
                title="Delete folder"
                onclick={() => (deleteOpen = true)}
              >
                <Trash size={15} />
                Delete
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
