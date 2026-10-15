<script lang="ts">
  import { ChevronRight, RotateCcw } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { trackDeletion } from "../deletion.svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { FileTree, Root } from "../types";
  import { cn } from "../utils";
  import DeletionBanner from "./DeletionBanner.svelte";
  import FolderPicker from "./FolderPicker.svelte";
  import TrashTreeItem from "./TrashTreeItem.svelte";
  import Button from "./ui/button/button.svelte";
  import ConfirmDialog from "./ui/confirm-dialog/confirm-dialog.svelte";

  let { roots }: { roots: Root[] } = $props();

  let openRoots = $state<Record<string, boolean>>({});
  let trees = $state<Record<string, FileTree>>({});
  let loading = $state<Record<string, boolean>>({});
  let expanded = $state<Record<string, boolean>>({});

  let picker = $state<
    | { kind: "item"; root: Root; node: FileTree }
    | { kind: "all"; root: Root }
    | null
  >(null);
  let selectedTree = $state<FileTree | null>(null);
  let deleteTarget = $state<{ root: Root; node: FileTree } | null>(null);
  let deleting = $state(false);

  const deleteDescription = $derived(
    deleteTarget?.node.kind === "directory"
      ? "This folder and everything inside it will be permanently deleted. This cannot be undone."
      : "This file will be permanently deleted. This cannot be undone.",
  );

  const pickerTitle = $derived(
    picker?.kind === "all"
      ? "Restore all"
      : picker?.kind === "item"
        ? `Restore ${picker.node.name}`
        : "",
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

  async function loadPicker(
    next: { kind: "item"; root: Root; node: FileTree } | { kind: "all"; root: Root },
  ) {
    picker = next;
    selectedTree = null;

    try {
      selectedTree = await session.client.tree(next.root.id);
    } catch {
      picker = null;
      toastError("Could not load folders");
    }
  }

  async function openPicker(root: Root, node: FileTree) {
    await loadPicker({ kind: "item", root, node });
  }

  async function openRestoreAll(root: Root) {
    await loadPicker({ kind: "all", root });
  }

  function closePicker() {
    picker = null;
    selectedTree = null;
  }

  async function confirmPlace(parent: string | null) {
    if (!picker) {
      return;
    }

    const rootId = picker.root.id;

    try {
      if (picker.kind === "all") {
        await session.client.restoreAll(rootId, parent);
      } else {
        await session.client.restore(rootId, picker.node.id, parent);
      }
    } catch {
      toastError(
        picker.kind === "all"
          ? "Could not restore trash"
          : "Could not restore this item",
      );
      return;
    }

    closePicker();
    await reload(rootId);
  }

  async function confirmDelete() {
    if (!deleteTarget || deleting) {
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
  <ul class="divide-y">
    {#each nodes as node (node.id)}
      <li>
        <TrashTreeItem
          {root}
          {node}
          {depth}
          expanded={!!expanded[node.id]}
          onToggle={() => toggleNode(node.id)}
          onRestore={() => openPicker(root, node)}
          onDelete={() => (deleteTarget = { root, node })}
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
            <button
              type="button"
              class="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
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
            </button>

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
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="h-8 shrink-0 gap-1.5 text-muted-foreground"
                onclick={() => openRestoreAll(root)}
              >
                <RotateCcw size={14} />
                Restore all
              </Button>
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

{#if selectedTree != null}
  <FolderPicker
    enabled={picker !== null}
    tree={selectedTree}
    rootName={picker?.root.name ?? ""}
    title={pickerTitle}
    description="Choose a folder"
    onCancel={closePicker}
    onConfirm={confirmPlace}
  />
{/if}

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
