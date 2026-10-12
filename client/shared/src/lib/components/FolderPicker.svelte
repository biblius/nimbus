<script lang="ts">
  import { ChevronRight, Folder } from "@lucide/svelte";
  import type { FileTree } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";

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
  let dialog = $state<HTMLDivElement | null>(null);
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

  $effect(() => {
    if (!enabled) {
      return;
    }

    dialog?.focus();

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        onCancel();
      }
    };

    window.addEventListener("keydown", onKeydown);

    return () => window.removeEventListener("keydown", onKeydown);
  });

  function cancel() {
    if (busy) {
      return;
    }

    onCancel();
  }

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
      {#if node.kind === "directory"}
        <li>
          <div
            class="flex h-7 items-center pr-1"
            style:padding-left="{depth * 0.75}rem"
          >
            {#if node.children.some((child) => child.kind === "directory")}
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
            {@render rows(node.children, depth + 1)}
          {/if}
        </li>
      {/if}
    {/each}
  </ul>
{/snippet}

{#if enabled}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <button
      type="button"
      class="absolute inset-0 bg-black/60"
      aria-label="Close"
      onclick={cancel}
      disabled={busy}
    ></button>
    <div
      bind:this={dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby="folder-picker-title"
      tabindex="-1"
      class="relative flex max-h-[min(32rem,80dvh)] w-full max-w-md flex-col rounded-xl border bg-card p-6 shadow-lg outline-none"
    >
      <h2 id="folder-picker-title" class="text-base font-medium">{title}</h2>
      <p class="mt-2 text-sm text-muted-foreground">{description}</p>
      <div
        class="mt-4 min-h-0 flex-1 overflow-y-auto rounded-md border p-3"
      >
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
        {@render rows(tree.children, 1)}
      </div>
      <div class="mt-5 flex justify-end gap-2">
        <Button type="button" variant="outline" onclick={cancel} disabled={busy}
          >Cancel</Button
        >
        <Button type="button" onclick={confirm} disabled={busy}
          >{confirmLabel}</Button
        >
      </div>
    </div>
  </div>
{/if}
