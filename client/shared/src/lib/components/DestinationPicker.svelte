<script lang="ts" module>
  import type { Root } from "../types";

  export type DestinationRequest = {
    roots: Root[];
    title: string;
    /** Defaults to "Choose a folder". */
    description?: string;
    confirmLabel: string;
    /** Defaults to "Could not load folders". */
    loadError?: string;
    onConfirm: (root: Root, parent: string | null) => void | Promise<void>;
  };
</script>

<script lang="ts">
  import { ChevronRight, Folder } from "@lucide/svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { FileTree, Root } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";
  import Dialog from "./ui/dialog/dialog.svelte";

  let {
    request,
    onCancel,
  }: {
    request: DestinationRequest | null;
    onCancel: () => void;
  } = $props();

  let expandedRoots = $state<Record<string, boolean>>({});
  let trees = $state<Record<string, FileTree>>({});
  let loadingRoots = $state<Record<string, boolean>>({});
  let expandedDirs = $state<Record<string, boolean>>({});
  let selection = $state<{ rootId: string; parent: string | null } | null>(
    null,
  );
  let busy = $state(false);
  let started = false;
  let activeRequest: DestinationRequest | null = null;

  const open = $derived(request != null && request.roots.length > 0);
  const description = $derived(request?.description ?? "Choose a folder");

  $effect(() => {
    const next = request;

    if (next == null) {
      started = false;
      activeRequest = null;
      resetState();
      return;
    }

    if (activeRequest !== next) {
      started = false;
      resetState();
      activeRequest = next;
    }

    if (started) {
      return;
    }

    started = true;

    if (next.roots.length === 0) {
      toastError("No roots are available");
      onCancel();
    }
  });

  function resetState() {
    expandedRoots = {};
    trees = {};
    loadingRoots = {};
    expandedDirs = {};
    selection = null;
    busy = false;
  }

  async function toggleRoot(root: Root) {
    const next = !expandedRoots[root.id];

    expandedRoots[root.id] = next;

    if (!next || trees[root.id]) {
      return;
    }

    loadingRoots[root.id] = true;

    try {
      trees[root.id] = await session.client.tree(root.id);
    } catch {
      expandedRoots[root.id] = false;
      toastError(request?.loadError ?? "Could not load folders");
    } finally {
      loadingRoots[root.id] = false;
    }
  }

  function toggleDir(nodeId: string) {
    expandedDirs[nodeId] = !expandedDirs[nodeId];
  }

  function selectRoot(root: Root) {
    selection = { rootId: root.id, parent: null };
  }

  function selectDir(root: Root, nodeId: string) {
    selection = { rootId: root.id, parent: nodeId };
  }

  function isSelected(rootId: string, parent: string | null) {
    return (
      selection != null &&
      selection.rootId === rootId &&
      selection.parent === parent
    );
  }

  async function confirm() {
    if (!request || !selection || busy) {
      return;
    }

    const root = request.roots.find((item) => item.id === selection!.rootId);

    if (!root) {
      return;
    }

    busy = true;

    try {
      await request.onConfirm(root, selection.parent);
    } finally {
      busy = false;
    }
  }
</script>

{#snippet dirRows(nodes: FileTree[], root: Root, depth: number)}
  {@const dirs = nodes.filter((node) => node.kind === "directory")}
  {#if dirs.length === 0}
    <p
      class="px-2 py-2 text-sm text-muted-foreground"
      style:padding-left="{depth * 0.875}rem"
    >
      No folders
    </p>
  {:else}
    <ul class="flex flex-col gap-0.5">
      {#each dirs as node (node.id)}
        {@const children = node.children ?? []}
        {@const dirExpanded = !!expandedDirs[node.id]}
        {@const hasChildDirs = children.some(
          (child) => child.kind === "directory",
        )}
        <li>
          <div
            class="flex h-9 items-center pr-1"
            style:padding-left="{depth * 0.875}rem"
          >
            {#if hasChildDirs}
              <button
                type="button"
                class="inline-flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-expanded={dirExpanded ? "true" : "false"}
                aria-label={dirExpanded
                  ? `Collapse ${node.name}`
                  : `Expand ${node.name}`}
                disabled={busy}
                onclick={() => toggleDir(node.id)}
              >
                <ChevronRight
                  size={14}
                  class={cn(
                    "transition-transform",
                    dirExpanded && "rotate-90",
                  )}
                />
              </button>
            {:else}
              <span class="size-6 shrink-0"></span>
            {/if}
            <button
              type="button"
              class={cn(
                "flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left text-sm",
                isSelected(root.id, node.id)
                  ? "bg-accent font-medium text-accent-foreground"
                  : "hover:bg-accent/60",
              )}
              aria-pressed={isSelected(root.id, node.id)}
              disabled={busy}
              onclick={() => selectDir(root, node.id)}
            >
              <Folder size={16} class="shrink-0 text-primary" />
              <span class="truncate">{node.name}</span>
            </button>
          </div>
          {#if dirExpanded}
            {@render dirRows(children, root, depth + 1)}
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

{#if request && open}
  <Dialog
    open={true}
    title={request.title}
    {description}
    {busy}
    {onCancel}
  >
    <div class="mt-4 max-h-72 overflow-y-auto rounded-md border p-2.5">
      <ul class="flex flex-col gap-0.5">
        {#each request.roots as root (root.id)}
          {@const tree = trees[root.id]}
          {@const rootExpanded = !!expandedRoots[root.id]}
          {@const rootLoading = !!loadingRoots[root.id]}
          <li>
            <div class="flex h-9 items-center pr-1">
              <button
                type="button"
                class="inline-flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-expanded={rootExpanded ? "true" : "false"}
                aria-label={rootExpanded
                  ? `Collapse ${root.name}`
                  : `Expand ${root.name}`}
                disabled={busy}
                onclick={(event) => {
                  event.stopPropagation();
                  void toggleRoot(root);
                }}
              >
                <ChevronRight
                  size={14}
                  class={cn(
                    "transition-transform",
                    rootExpanded && "rotate-90",
                  )}
                />
              </button>
              <button
                type="button"
                class={cn(
                  "flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left text-sm",
                  isSelected(root.id, null)
                    ? "bg-accent font-medium text-accent-foreground"
                    : "hover:bg-accent/60",
                )}
                aria-pressed={isSelected(root.id, null)}
                disabled={busy}
                onclick={() => selectRoot(root)}
              >
                <Folder size={16} class="shrink-0 text-primary" />
                <span class="truncate">{root.name}</span>
              </button>
            </div>
            {#if rootExpanded}
              {#if rootLoading}
                <p
                  class="px-2 py-2 text-sm text-muted-foreground"
                  style:padding-left="0.875rem"
                >
                  Loading…
                </p>
              {:else if tree}
                {@render dirRows(tree.children ?? [], root, 1)}
              {/if}
            {/if}
          </li>
        {/each}
      </ul>
    </div>
    {#snippet footer()}
      <Button
        type="button"
        variant="outline"
        onclick={onCancel}
        disabled={busy}
      >
        Cancel
      </Button>
      <Button
        type="button"
        onclick={confirm}
        disabled={busy || selection == null}
      >
        {request.confirmLabel}
      </Button>
    {/snippet}
  </Dialog>
{/if}
