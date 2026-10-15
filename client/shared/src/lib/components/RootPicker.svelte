<script lang="ts">
  import { Folder } from "@lucide/svelte";
  import type { Root } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";
  import Dialog from "./ui/dialog/dialog.svelte";

  let {
    open,
    roots,
    title,
    description,
    confirmLabel = "Confirm",
    busy = false,
    onCancel,
    onConfirm,
  }: {
    open: boolean;
    roots: Root[];
    title: string;
    description: string;
    confirmLabel?: string;
    busy?: boolean;
    onCancel: () => void;
    onConfirm: (rootId: string) => void | Promise<void>;
  } = $props();

  let selected = $state<string | null>(null);
  let opened = false;

  const privateRoots = $derived(roots.filter((root) => root.owner != null));
  const sharedRoots = $derived(roots.filter((root) => root.owner == null));

  $effect(() => {
    if (!open) {
      opened = false;
      return;
    }

    if (!opened) {
      opened = true;
      selected = null;
    }
  });

  async function confirm() {
    if (busy || selected === null) {
      return;
    }

    await onConfirm(selected);
  }
</script>

{#snippet group(label: string, items: Root[], separated: boolean)}
  <p
    class={cn(
      "px-1.5 py-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase",
      separated && "mt-2 border-t pt-2",
    )}
  >
    {label}
  </p>
  <ul class="flex flex-col gap-0.5">
    {#each items as root (root.id)}
      <li>
        <button
          type="button"
          class={cn(
            "flex h-9 w-full min-w-0 items-center gap-2 rounded-md px-2 text-left text-sm",
            selected === root.id
              ? "bg-accent font-medium text-accent-foreground"
              : "hover:bg-accent/60",
          )}
          aria-pressed={selected === root.id}
          disabled={busy}
          onclick={() => (selected = root.id)}
        >
          <Folder size={16} class="shrink-0 text-primary" />
          <span class="truncate">{root.name}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<Dialog {open} {title} {description} {busy} {onCancel}>
  <div class="mt-4 min-h-0 flex-1 overflow-y-auto rounded-md border p-2">
    {#if roots.length === 0}
      <p class="px-2 py-3 text-sm text-muted-foreground">
        No roots are available.
      </p>
    {:else}
      {#if privateRoots.length > 0}
        {@render group("Private", privateRoots, false)}
      {/if}
      {#if sharedRoots.length > 0}
        {@render group("Shared", sharedRoots, privateRoots.length > 0)}
      {/if}
    {/if}
  </div>
  {#snippet footer()}
    <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>
      Cancel
    </Button>
    <Button type="button" onclick={confirm} disabled={busy || selected === null}>
      {confirmLabel}
    </Button>
  {/snippet}
</Dialog>
