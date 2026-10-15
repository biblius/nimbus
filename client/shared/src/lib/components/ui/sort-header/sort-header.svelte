<script lang="ts">
  import { ArrowDown, ArrowUp } from "@lucide/svelte";
  import { cn } from "../../../utils";

  let {
    label,
    direction,
    onclick,
    class: className,
    buttonClass,
  }: {
    label: string;
    /** Set when the list is sorted by this column. */
    direction?: "asc" | "desc";
    onclick: () => void;
    /** Classes for the header cell. */
    class?: string;
    /** Classes for the inner button, e.g. alignment. */
    buttonClass?: string;
  } = $props();

  const ariaSort = $derived(
    direction === "asc"
      ? "ascending"
      : direction === "desc"
        ? "descending"
        : "none",
  );
</script>

<div
  role="columnheader"
  aria-sort={ariaSort}
  class={cn("flex h-full min-w-0 items-center", className)}
>
  <button
    type="button"
    class={cn(
      "inline-flex h-full w-full min-w-0 items-center gap-1 hover:text-foreground",
      direction && "text-foreground",
      buttonClass,
    )}
    {onclick}
  >
    <span class="truncate">{label}</span>
    {#if direction === "asc"}
      <ArrowUp size={14} class="shrink-0" aria-hidden="true" />
    {:else if direction === "desc"}
      <ArrowDown size={14} class="shrink-0" aria-hidden="true" />
    {/if}
  </button>
</div>
