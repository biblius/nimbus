<script lang="ts">
  import { Eye, PenLine, Share2 } from "@lucide/svelte";
  import { AccessMode } from "../../types";

  const WRITE_TITLE =
    "You can upload, edit, move, rename, and trash items in this folder";
  const SHARE_TITLE =
    "You can create and manage public links for files in this folder";
  const VIEW_TITLE = "You can browse and download items in this folder";

  let { accessMode }: { accessMode: number } = $props();

  const canWrite = $derived((accessMode & AccessMode.WRITE) !== 0);
  const canShare = $derived((accessMode & AccessMode.SHARE) !== 0);
  const viewOnly = $derived(!canWrite && !canShare);
</script>

{#if canWrite || canShare || viewOnly}
  <div
    class="flex shrink-0 items-center gap-1"
    aria-label="Folder access"
  >
    {#if canWrite}
      <span
        class="inline-flex size-6 items-center justify-center rounded-md bg-amber-500/15 text-amber-700 dark:text-amber-400"
        title={WRITE_TITLE}
        aria-label={WRITE_TITLE}
      >
        <PenLine size={14} aria-hidden="true" />
      </span>
    {/if}
    {#if canShare}
      <span
        class="inline-flex size-6 items-center justify-center rounded-md bg-primary/10 text-primary"
        title={SHARE_TITLE}
        aria-label={SHARE_TITLE}
      >
        <Share2 size={14} aria-hidden="true" />
      </span>
    {/if}
    {#if viewOnly}
      <span
        class="inline-flex size-6 items-center justify-center rounded-md bg-muted text-muted-foreground"
        title={VIEW_TITLE}
        aria-label={VIEW_TITLE}
      >
        <Eye size={14} aria-hidden="true" />
      </span>
    {/if}
  </div>
{/if}
