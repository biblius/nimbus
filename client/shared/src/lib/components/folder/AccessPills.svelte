<script lang="ts">
  import { PenLine, Share2 } from "@lucide/svelte";
  import { AccessMode } from "../../types";
  import { cn } from "../../utils";

  let {
    accessMode,
    interactive = false,
    onToggle,
  }: {
    accessMode: number;
    /** When true, pills toggle flags via `onToggle`. */
    interactive?: boolean;
    onToggle?: (flag: number) => void;
  } = $props();

  const canWrite = $derived((accessMode & AccessMode.WRITE) !== 0);
  const canShare = $derived((accessMode & AccessMode.SHARE) !== 0);

  function pillClass(active: boolean, tone: "write" | "share") {
    if (!active) {
      return "bg-transparent text-muted-foreground ring-1 ring-border hover:bg-muted/60 hover:text-foreground";
    }
    if (tone === "write") {
      return "bg-amber-500/15 text-amber-700 ring-1 ring-amber-500/25 dark:text-amber-400";
    }
    return "bg-primary/10 text-primary ring-1 ring-primary/25";
  }
</script>

<div class="flex flex-wrap items-center gap-1.5">
  {#if interactive}
    <button
      type="button"
      class={cn(
        "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-colors",
        pillClass(canWrite, "write"),
      )}
      aria-pressed={canWrite}
      title="Can upload, edit, move, rename, and trash items"
      onclick={() => onToggle?.(AccessMode.WRITE)}
    >
      <PenLine size={12} aria-hidden="true" />
      Edit
    </button>
    <button
      type="button"
      class={cn(
        "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition-colors",
        pillClass(canShare, "share"),
      )}
      aria-pressed={canShare}
      title="Can create and manage public links"
      onclick={() => onToggle?.(AccessMode.SHARE)}
    >
      <Share2 size={12} aria-hidden="true" />
      Share
    </button>
  {:else}
    {#if canWrite}
      <span
        class={cn(
          "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium",
          pillClass(true, "write"),
        )}
        title="Can upload, edit, move, rename, and trash items"
      >
        <PenLine size={12} aria-hidden="true" />
        Can edit
      </span>
    {/if}
    {#if canShare}
      <span
        class={cn(
          "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium",
          pillClass(true, "share"),
        )}
        title="Can create and manage public links"
      >
        <Share2 size={12} aria-hidden="true" />
        Can share
      </span>
    {/if}
    {#if !canWrite && !canShare}
      <span
        class="inline-flex items-center rounded-md bg-muted px-2 py-1 text-xs font-medium text-muted-foreground"
      >
        View only
      </span>
    {/if}
  {/if}
</div>
