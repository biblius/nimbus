<script lang="ts">
  import { Folder } from "@lucide/svelte";
  import type { Root } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";

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
  let dialog = $state<HTMLDivElement | null>(null);
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
    if (busy || selected === null) {
      return;
    }

    await onConfirm(selected);
  }
</script>

{#if open}
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
      aria-labelledby="root-picker-title"
      tabindex="-1"
      class="relative flex max-h-[min(32rem,80dvh)] w-full max-w-md flex-col rounded-xl border bg-card p-6 shadow-lg outline-none"
    >
      <h2 id="root-picker-title" class="text-base font-medium">{title}</h2>
      <p class="mt-2 text-sm text-muted-foreground">{description}</p>
      <div class="mt-4 min-h-0 flex-1 overflow-y-auto rounded-md border p-2">
        {#if roots.length === 0}
          <p class="px-2 py-3 text-sm text-muted-foreground">
            No roots are available.
          </p>
        {:else}
          {#if privateRoots.length > 0}
            <p
              class="px-1.5 py-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase"
            >
              Private
            </p>
            <ul class="flex flex-col gap-0.5">
              {#each privateRoots as root (root.id)}
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
          {/if}
          {#if sharedRoots.length > 0}
            <p
              class={cn(
                "px-1.5 py-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase",
                privateRoots.length > 0 && "mt-2 border-t pt-2",
              )}
            >
              Shared
            </p>
            <ul class="flex flex-col gap-0.5">
              {#each sharedRoots as root (root.id)}
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
          {/if}
        {/if}
      </div>
      <div class="mt-5 flex justify-end gap-2">
        <Button type="button" variant="outline" onclick={cancel} disabled={busy}>
          Cancel
        </Button>
        <Button
          type="button"
          onclick={confirm}
          disabled={busy || selected === null}
        >
          {confirmLabel}
        </Button>
      </div>
    </div>
  </div>
{/if}
