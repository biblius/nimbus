<script lang="ts">
  import type { PendingUpload } from "../../pending-upload";

  let {
    item,
    percent,
    onCancel,
    onDismiss,
  }: {
    item: PendingUpload;
    percent: number;
    onCancel: () => void;
    onDismiss: () => void;
  } = $props();
</script>

<li class="space-y-1">
  <div class="flex items-center justify-between gap-3 text-sm">
    <span class="min-w-0 truncate">{item.name}</span>
    {#if item.error}
      <span class="flex shrink-0 items-center gap-2 text-destructive">
        {item.error}
        <button type="button" class="text-xs underline" onclick={onDismiss}>
          Dismiss
        </button>
      </span>
    {:else}
      <span class="flex shrink-0 items-center gap-2">
        <span class="text-muted-foreground tabular-nums">{percent}%</span>
        <button
          type="button"
          class="text-xs underline"
          aria-label="Cancel {item.name}"
          onclick={onCancel}
        >
          Cancel
        </button>
      </span>
    {/if}
  </div>
  <div
    class="h-1.5 overflow-hidden rounded-full bg-muted"
    role="progressbar"
    aria-label={item.name}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
  >
    <div
      class="h-full rounded-full transition-[width] duration-150 {item.error
        ? 'bg-destructive'
        : 'bg-primary'}"
      style:width="{percent}%"
    ></div>
  </div>
</li>
