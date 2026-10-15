<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "../../../utils";

  let {
    open,
    title,
    description,
    busy = false,
    class: className,
    onCancel,
    children,
    footer,
  }: {
    open: boolean;
    title: string;
    description?: string;
    /** Blocks closing while an action is running. */
    busy?: boolean;
    /** Classes for the dialog panel. */
    class?: string;
    onCancel: () => void;
    children?: Snippet;
    footer?: Snippet;
  } = $props();

  const titleId = $props.id();

  let panel = $state<HTMLDivElement>();

  function cancel() {
    if (!busy) {
      onCancel();
    }
  }

  $effect(() => {
    if (!open) {
      return;
    }

    panel?.focus();

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        cancel();
      }
    };

    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });
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
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
      class={cn(
        "relative flex max-h-[min(32rem,85dvh)] w-full max-w-md flex-col rounded-xl border bg-card p-6 shadow-lg outline-none",
        className,
      )}
    >
      <h2 id={titleId} class="text-base font-medium">{title}</h2>
      {#if description}
        <p class="mt-2 text-sm text-muted-foreground">{description}</p>
      {/if}
      {@render children?.()}
      {#if footer}
        <div class="mt-5 flex justify-end gap-2">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}
