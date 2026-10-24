<script lang="ts">
  import { ChevronRight, RotateCcw } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { trackDeletion } from "../deletion.svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import { hasWriteAccess, type FileTree, type Root } from "../types";
  import { cn } from "../utils";
  import DeletionBanner from "./DeletionBanner.svelte";
  import type { DestinationRequest } from "../file-browser/destination";
  import DestinationPicker from "./file-browser/DestinationPicker.svelte";
  import TrashTreeItem from "./TrashTreeItem.svelte";
  import Button from "./ui/button/button.svelte";
  import ConfirmDialog from "./ui/confirm-dialog/confirm-dialog.svelte";

  let { roots }: { roots: Root[] } = $props();

  let openRoots = $state<Record<string, boolean>>({});
  let trees = $state<Record<string, FileTree>>({});
  let loading = $state<Record<string, boolean>>({});
  let expanded = $state<Record<string, boolean>>({});

  let destination = $state<DestinationRequest | null>(null);
  let deleteTarget = $state<{ root: Root; node: FileTree } | null>(null);
  let deleting = $state(false);

  const deleteDescription = $derived(
    deleteTarget?.node.kind === "directory"
      ? "This folder and everything inside it will be permanently deleted. This cannot be undone."
      : "This file will be permanently deleted. This cannot be undone.",
  );

  onMount(() => {
    const owned = roots.find((root) => root.owner === session.user?.id);

    if (owned) {
      toggleRoot(owned);
    }
  });

  async function toggleRoot(root: Root) {
    const next = !openRoots[root.id];

    openRoots[root.id] = next;

    if (!next || trees[root.id]) {
      // Return if the root is already loaded
      return;
    }

    loading[root.id] = true;

    try {
      trees[root.id] = await session.client.trashTree(root.id);
    } catch {
      openRoots[root.id] = false;
      toastError("Could not load trash");
    } finally {
      loading[root.id] = false;
    }
  }

  async function reload(rootId: string) {
    trees[rootId] = await session.client.trashTree(rootId);
  }

  function toggleNode(id: string) {
    expanded[id] = !expanded[id];
  }

  function openPicker(root: Root, node: FileTree) {
    if (!hasWriteAccess(root)) {
      return;
    }

    destination = {
      roots: [root],
      title: `Restore ${node.name}`,
      description: "Choose a folder",
      confirmLabel: "Restore",
      onConfirm: async (_r, parent) => {
        try {
          await session.client.restore(root.id, node.id, parent);
          destination = null;
          await reload(root.id);
        } catch {
          toastError("Could not restore this item");
        }
      },
    };
  }

  function openRestoreAll(root: Root) {
    if (!hasWriteAccess(root)) {
      return;
    }

    destination = {
      roots: [root],
      title: "Restore all",
      description: "Choose a folder",
      confirmLabel: "Restore",
      onConfirm: async (_r, parent) => {
        try {
          await session.client.restoreAll(root.id, parent);
          destination = null;
          await reload(root.id);
        } catch {
          toastError("Could not restore trash");
        }
      },
    };
  }

  async function confirmDelete() {
    if (!deleteTarget || deleting || !hasWriteAccess(deleteTarget.root)) {
      return;
    }

    const { root, node } = deleteTarget;

    deleting = true;

    try {
      deleteTarget = null;
      await trackDeletion(session.client.deleteTrash(root.id, node.id));
      await reload(root.id);
    } catch {
      toastError("Could not permanently delete this item");
    } finally {
      deleting = false;
    }
  }
</script>

{#snippet rows(root: Root, nodes: FileTree[], depth: number)}
  {@const canWrite = hasWriteAccess(root)}
  <ul class="divide-y">
    {#each nodes as node (node.id)}
      <li>
        <TrashTreeItem
          {root}
          {node}
          {depth}
          {canWrite}
          expanded={!!expanded[node.id]}
          onToggle={() => toggleNode(node.id)}
          onRestore={() => openPicker(root, node)}
          onDelete={() => {
            if (canWrite) {
              deleteTarget = { root, node };
            }
          }}
        />
        {#if expanded[node.id] && node.children.length > 0}
          <div class="border-t">
            {@render rows(root, node.children, depth + 1)}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div class="flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-hidden">
  <h1 class="shrink-0 text-lg font-medium">Trash</h1>
  <div class="min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain">
    {#if roots.length === 0}
      <p class="text-sm text-muted-foreground">No roots are assigned.</p>
    {:else}
      {#each roots as root (root.id)}
        {@const trashTree = trees[root.id]}
        {@const trashCount = trashTree?.children.length ?? 0}
        {@const hasTrash = trashTree != null && trashCount > 0}
        <section
          class="overflow-hidden rounded-lg border bg-card shadow-sm ring-1 ring-border/40"
        >
          <header
            class="flex min-h-12 items-center gap-2 border-b bg-muted/40 px-2.5"
          >
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="shrink-0 text-muted-foreground hover:text-foreground"
              aria-expanded={openRoots[root.id] ? "true" : "false"}
              aria-label={openRoots[root.id]
                ? `Collapse ${root.name}`
                : `Expand ${root.name}`}
              onclick={() => toggleRoot(root)}
            >
              <ChevronRight
                size={16}
                class={cn(
                  "transition-transform",
                  openRoots[root.id] && "rotate-90",
                )}
              />
            </Button>

            <div class="flex min-w-0 flex-1 items-center gap-2">
              <span class="truncate text-sm font-medium tracking-tight">
                {root.name}
              </span>
              <span
                class="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-medium tracking-wide text-muted-foreground uppercase"
              >
                {root.owner ? "Private" : "Shared"}
              </span>
            </div>

            {#if trashTree}
              <span
                class="shrink-0 rounded-md bg-background/80 px-1.5 py-0.5 text-xs font-medium text-muted-foreground tabular-nums ring-1 ring-border/60"
              >
                {trashCount}
              </span>
            {/if}

            {#if hasTrash}
              {@const canWrite = hasWriteAccess(root)}
              <span
                class="inline-flex"
                title={canWrite ? undefined : "You do not have write access"}
              >
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="h-8 shrink-0 gap-1.5 text-muted-foreground"
                  disabled={!canWrite}
                  aria-label={canWrite
                    ? "Restore all"
                    : "You do not have write access"}
                  onclick={() => openRestoreAll(root)}
                >
                  <RotateCcw size={14} />
                  Restore all
                </Button>
              </span>
            {/if}
          </header>

          {#if openRoots[root.id]}
            {#if loading[root.id]}
              <p class="px-3 py-8 text-center text-sm text-muted-foreground">
                Loading…
              </p>
            {:else if !trashTree || trashCount === 0}
              <p class="px-3 py-10 text-center text-sm text-muted-foreground">
                Trash is empty
              </p>
            {:else}
              <div class="bg-card">
                {@render rows(root, trashTree.children, 0)}
              </div>
            {/if}
          {/if}
        </section>
      {/each}
    {/if}
  </div>
</div>

<DestinationPicker
  request={destination}
  onCancel={() => (destination = null)}
/>

<ConfirmDialog
  open={deleteTarget !== null}
  title={deleteTarget ? `Permanently delete ${deleteTarget.node.name}?` : ""}
  description={deleteDescription}
  confirmLabel="Delete permanently"
  busy={deleting}
  onCancel={() => {
    if (!deleting) {
      deleteTarget = null;
    }
  }}
  onConfirm={confirmDelete}
/>

<DeletionBanner />
