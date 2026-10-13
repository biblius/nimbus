<script lang="ts">
  import { deletionBanner } from "../deletion.svelte";
</script>

{#if deletionBanner.open}
  <div
    class="pointer-events-none fixed right-4 bottom-4 z-50 w-[min(100vw-2rem,22rem)] rounded-lg border bg-card p-3 shadow-lg"
    role="status"
    aria-live="polite"
  >
    <div class="mb-1.5 flex items-center justify-between gap-3 text-sm">
      <span class="min-w-0 truncate">{deletionBanner.label}</span>
      <span
        class="shrink-0 tabular-nums text-muted-foreground"
        class:text-destructive={deletionBanner.failed}
      >
        {deletionBanner.percent}%
      </span>
    </div>
    <div
      class="h-1.5 overflow-hidden rounded-full bg-muted"
      role="progressbar"
      aria-label={deletionBanner.label}
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={deletionBanner.percent}
    >
      <div
        class="h-full rounded-full transition-[width] duration-150 {deletionBanner.failed
          ? 'bg-destructive'
          : 'bg-primary'}"
        style:width="{deletionBanner.percent}%"
      ></div>
    </div>
  </div>
{/if}
