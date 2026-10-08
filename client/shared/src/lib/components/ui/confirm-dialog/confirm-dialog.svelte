<script lang="ts">
  import Button from "../button/button.svelte";

  let {
    open,
    title,
    description,
    confirmLabel = "Delete",
    busy = false,
    onCancel,
    onConfirm,
  }: {
    open: boolean;
    title: string;
    description: string;
    confirmLabel?: string;
    busy?: boolean;
    onCancel: () => void;
    onConfirm: () => void;
  } = $props();

  let dialog = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!open) return;
    dialog?.focus();
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <button
      type="button"
      class="absolute inset-0 bg-black/40"
      aria-label="Close"
      onclick={onCancel}
      disabled={busy}
    ></button>
    <div
      bind:this={dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-title"
      tabindex="-1"
      class="relative w-full max-w-md rounded-xl border bg-card p-5 shadow-lg outline-none"
    >
      <h2 id="confirm-dialog-title" class="text-base font-medium">{title}</h2>
      <p class="mt-2 text-sm text-muted-foreground">{description}</p>
      <div class="mt-5 flex justify-end gap-2">
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
          variant="destructive"
          onclick={onConfirm}
          disabled={busy}
        >
          {confirmLabel}
        </Button>
      </div>
    </div>
  </div>
{/if}
