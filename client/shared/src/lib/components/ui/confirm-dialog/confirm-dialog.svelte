<script lang="ts">
  import Button from "../button/button.svelte";
  import Dialog from "../dialog/dialog.svelte";

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
</script>

<Dialog {open} {title} {description} {busy} {onCancel}>
  {#snippet footer()}
    <Button type="button" variant="outline" onclick={onCancel} disabled={busy}>
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
  {/snippet}
</Dialog>
