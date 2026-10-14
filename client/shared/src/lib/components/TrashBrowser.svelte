<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { FileTree, Root } from "../types";
  import { cn } from "../utils";
  import FolderPicker from "./FolderPicker.svelte";
  import TrashTreeItem from "./TrashTreeItem.svelte";

  let { roots }: { roots: Root[] } = $props();

  let openRoots = $state<Record<string, boolean>>({});
  let trees = $state<Record<string, FileTree>>({});
  let loading = $state<Record<string, boolean>>({});
  let expanded = $state<Record<string, boolean>>({});

  let picker = $state<{ root: Root; node: FileTree } | null>(null);
  let selectedTree = $state<FileTree | null>(null);

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

  async function openPicker(root: Root, node: FileTree) {
    picker = { root, node };
    selectedTree = null;

    try {
      selectedTree = await session.client.tree(root.id);
    } catch {
      picker = null;
      toastError("Could not load folders");
    }
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
    const nodeId = picker.node.id;

    try {
      await session.client.restore(rootId, nodeId, parent);
    } catch {
      toastError("Could not restore this item");
      return;
    }

    closePicker();
    await reload(rootId);
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
        <section class="overflow-hidden rounded-lg border bg-card">
          <button
            type="button"
            class="flex min-h-11 w-full items-center gap-2.5 px-3 text-left text-sm font-medium"
            aria-expanded={openRoots[root.id] ? "true" : "false"}
            onclick={() => toggleRoot(root)}
          >
            <ChevronRight
              size={16}
              class={cn(
                "shrink-0 text-muted-foreground transition-transform",
                openRoots[root.id] && "rotate-90",
              )}
            />
            <span class="truncate">{root.name}</span>
            <span
              class="text-xs font-normal tracking-wide text-muted-foreground uppercase"
            >
              {root.owner ? "Private" : "Shared"}
            </span>
            {#if trees[root.id]}
              <span
                class="ml-auto text-xs font-normal text-muted-foreground tabular-nums"
              >
                {trees[root.id].children.length}
              </span>
            {/if}
          </button>

          {#if openRoots[root.id]}
            <div class="border-t">
              {#if loading[root.id]}
                <p class="px-3 py-6 text-center text-sm text-muted-foreground">
                  Loading…
                </p>
              {:else if (trees[root.id]?.children.length ?? 0) === 0}
                <p class="px-3 py-8 text-center text-sm text-muted-foreground">
                  Trash is empty
                </p>
              {:else}
                {@render rows(root, trees[root.id].children, 0)}
              {/if}
            </div>
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
    title={picker ? `Restore ${picker.node.name}` : ""}
    description="Choose a folder"
    onCancel={closePicker}
    onConfirm={confirmPlace}
  />
{/if}
