<script lang="ts">
  import { ChevronRight, Folder } from "@lucide/svelte";
  import type { FileTree } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";
  import Dialog from "./ui/dialog/dialog.svelte";

  let {
    enabled,
    tree,
    rootName,
    title,
    description,
    confirmLabel = "Restore",
    onCancel,
    onConfirm,
  }: {
    enabled: boolean;
    tree: FileTree;
    rootName: string;
    title: string;
    description: string;
    confirmLabel?: string;
    onCancel: () => void;
    onConfirm: (parent: string | null) => void | Promise<void>;
  } = $props();

  let location = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});
  let busy = $state(false);
  let opened = false;

  $effect(() => {
    if (!enabled) {
      opened = false;
      return;
    }

    if (opened) {
      return;
    }

    opened = true;
    location = null;
    expanded = {};
  });

  async function confirm() {
    if (busy) {
      return;
    }

    busy = true;

    try {
      await onConfirm(location);
    } finally {
      busy = false;
    }
  }
</script>

{#snippet rows(nodes: FileTree[], depth: number)}
  <ul class="flex flex-col">
    {#each nodes as node (node.id)}
      {@const children = node.children ?? []}
      {#if node.kind === "directory"}
        <li>
          <div
            class="flex h-7 items-center pr-1"
            style:padding-left="{depth * 0.75}rem"
          >
            {#if children.some((child) => child.kind === "directory")}
              <button
                type="button"
                class="inline-flex size-4 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-expanded={expanded[node.id] ? "true" : "false"}
                aria-label={expanded[node.id]
                  ? `Collapse ${node.name}`
                  : `Expand ${node.name}`}
                onclick={() => (expanded[node.id] = !expanded[node.id])}
              >
                <ChevronRight
                  size={12}
                  class={cn(
                    "transition-transform",
                    expanded[node.id] && "rotate-90",
                  )}
                />
              </button>
            {:else}
              <span class="size-4 shrink-0"></span>
            {/if}
            <button
              type="button"
              class={cn(
                "flex h-7 min-w-0 flex-1 items-center gap-1.5 rounded px-1.5 text-left text-[13px]",
                location === node.id
                  ? "bg-accent font-medium text-accent-foreground"
                  : "hover:bg-accent/60",
              )}
              aria-pressed={location === node.id}
              onclick={() => (location = node.id)}
            >
              <Folder size={14} class="shrink-0 text-primary" />
              <span class="truncate">{node.name}</span>
            </button>
          </div>
          {#if expanded[node.id]}
            {@render rows(children, depth + 1)}
          {/if}
        </li>
      {/if}
    {/each}
  </ul>
{/snippet}

<Dialog open={enabled} {title} {description} {busy} {onCancel}>
  <div class="mt-4 max-h-64 overflow-y-auto rounded-md border p-3">
    <div class="flex h-7 items-center pr-1">
      <span class="size-4 shrink-0"></span>
      <button
        type="button"
        class={cn(
          "flex h-7 min-w-0 flex-1 items-center gap-1.5 rounded px-1.5 text-left text-[13px]",
          location === null
            ? "bg-accent font-medium text-accent-foreground"
            : "hover:bg-accent/60",
        )}
        aria-pressed={location === null}
        onclick={() => (location = null)}
      >
        <Folder size={14} class="shrink-0 text-primary" />
        <span class="truncate">{rootName}</span>
      </button>
    </div>
    {@render rows(tree.children ?? [], 1)}
  </div>
  {#snippet footer()}
    <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>
      Cancel
    </Button>
    <Button type="button" onclick={confirm} disabled={busy}>
      {confirmLabel}
    </Button>
  {/snippet}
</Dialog>
