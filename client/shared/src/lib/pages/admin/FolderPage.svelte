<script lang="ts">
  import { Pencil, Trash } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { ApiError } from "../../api";
  import MemberInfo from "../../components/folder/MemberInfo.svelte";
  import RootForm from "../../components/folder/RootForm.svelte";
  import RootInfo from "../../components/folder/RootInfo.svelte";
  import Button from "../../components/ui/button/button.svelte";
  import ConfirmDialog from "../../components/ui/confirm-dialog/confirm-dialog.svelte";
  import { formatBytes, formatQuota } from "../../format";
  import { navigate, route } from "../../router";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Member, Root, RootData } from "../../types";

  /**
   * The currently viewed folder.
   */
  let folder = $state<Root | null>(null);

  /**
   * Whether the folder is shared, determined by its owner field.
   */
  let shared = $derived(folder != null && folder.owner == null);

  let notFound = $state(false);
  let failed = $state(false);
  let members = $state<Member[] | null>(null);
  let editing = $state(false);
  let deleteOpen = $state(false);
  let deleting = $state(false);
  /**
   * Bytes used.
   * Missing while the request is in flight; `null` if it failed.
   */
  let usage = $state<number | null | undefined>(undefined);

  function listHref() {
    return shared ? "/admin/folders/shared" : "/admin/folders/private";
  }

  function usageText() {
    if (usage === undefined) return "…";
    return usage == null ? "-" : formatBytes(usage);
  }

  async function load(id: string) {
    notFound = false;
    failed = false;
    folder = null;
    editing = false;
    deleteOpen = false;
    members = null;
    usage = undefined;

    try {
      const loaded = await session.client.getFolder(id);

      if (route.params.folderId !== id) return;

      folder = loaded;

      session.client
        .folderUsage(id)
        .then((bytes) => {
          if (route.params.folderId === id) usage = bytes;
        })
        .catch(() => {
          if (route.params.folderId === id) usage = null;
        });
    } catch (cause) {
      if (route.params.folderId !== id) return;

      if (cause instanceof ApiError && cause.status === 404) {
        notFound = true;
        return;
      }

      failed = true;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load the folder",
      );
    }
  }

  $effect(() => {
    const id = route.params.folderId;

    if (!id) return;

    untrack(() => load(id));
  });

  async function updateRoot(data: RootData) {
    if (!folder) return;

    const updated = await session.client.updateFolder(folder.id, data);

    folder = updated;
    members = null;
    editing = false;
  }

  async function confirmDelete() {
    if (!folder || deleting) return;

    deleting = true;

    try {
      await session.client.deleteFolder(folder.id);
      navigate(listHref());
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

<div class="space-y-3">
  <div class="flex h-12 items-center justify-between gap-3 border-b px-1">
    <h2 class="min-w-0 truncate text-sm font-medium">
      {#if folder}
        <a
          href={listHref()}
          class="text-muted-foreground hover:text-foreground"
        >
          {shared ? "Shared folders" : "Private folders"}
        </a>
        <span class="text-muted-foreground"> / </span>
        {folder.name}
      {/if}
    </h2>
    <span class="text-xs text-muted-foreground">
      {#if folder}
        {usageText()} / {formatQuota(folder.quota)}
      {/if}
    </span>
  </div>

  {#if notFound}
    <p class="text-sm text-muted-foreground">This folder does not exist.</p>
  {:else if failed}
    <p class="text-sm text-muted-foreground">Could not load the folder.</p>
  {:else if !folder}
    <p class="text-sm text-muted-foreground">Loading…</p>
  {:else if editing}
    <div class="mx-auto max-w-lg">
      <RootForm
        {folder}
        members={members ?? []}
        title="Edit folder"
        description={shared
          ? "Choose who can use this shared folder."
          : "A private folder stays with its owner."}
        save={updateRoot}
        cancel={() => (editing = false)}
      />
    </div>
  {:else}
    <div class="rounded-lg border bg-muted/30 p-3">
      <div class="flex justify-between gap-3">
        <div class="min-w-0 flex-1 space-y-4">
          <RootInfo {folder} usage={usageText()} />

          {#if shared}
            <MemberInfo folderId={folder.id} bind:members />
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
            disabled={shared && members == null}
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
    </div>
  {/if}
</div>

<ConfirmDialog
  open={deleteOpen}
  title="Delete folder"
  description={`Delete ${folder?.name}? This removes the folder and everything in it.`}
  busy={deleting}
  onCancel={() => {
    if (!deleting) deleteOpen = false;
  }}
  onConfirm={confirmDelete}
/>
