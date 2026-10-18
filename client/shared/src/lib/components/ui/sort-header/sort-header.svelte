<script lang="ts" module>
  export type SortDirection = "asc" | "desc";

  export type SortState<C extends string = string> = {
    column: C;
    direction: SortDirection;
  };

  /** none → asc → desc → defaultSort */
  export function cycleSortState<C extends string>(
    sort: SortState<C>,
    column: C,
    defaultSort: SortState<C>,
  ): SortState<C> {
    if (sort.column !== column) {
      return { column, direction: "asc" };
    }
    if (sort.direction === "asc") {
      return { column, direction: "desc" };
    }
    return { ...defaultSort };
  }
</script>

<script lang="ts" generics="Column extends string = string">
  import { ArrowDown, ArrowUp } from "@lucide/svelte";
  import { cn } from "../../../utils";

  let {
    label,
    column,
    sort = $bindable(),
    defaultSort,
    onCycle,
    class: className,
    buttonClass,
  }: {
    label: string;
    column: Column;
    sort: SortState<Column>;
    defaultSort: SortState<Column>;
    /** Called after sort is cycled (e.g. persist, reorder). */
    onCycle?: (column: Column) => void;
    /** Classes for the header cell. */
    class?: string;
    /** Classes for the inner button, e.g. alignment. */
    buttonClass?: string;
  } = $props();

  const direction = $derived(
    sort.column === column ? sort.direction : undefined,
  );

  const ariaLabel = $derived(
    direction === "asc"
      ? `${label}, sorted ascending`
      : direction === "desc"
        ? `${label}, sorted descending`
        : `Sort by ${label}`,
  );

  function handleClick() {
    sort = cycleSortState(sort, column, defaultSort);
    onCycle?.(column);
  }
</script>

<div class={cn("flex h-full min-w-0 items-center", className)}>
  <button
    type="button"
    aria-label={ariaLabel}
    class={cn(
      "inline-flex h-full w-full min-w-0 items-center gap-1 hover:text-foreground",
      direction && "text-foreground",
      buttonClass,
    )}
    onclick={handleClick}
  >
    <span class="truncate">{label}</span>
    {#if direction === "asc"}
      <ArrowUp size={14} class="shrink-0" aria-hidden="true" />
    {:else if direction === "desc"}
      <ArrowDown size={14} class="shrink-0" aria-hidden="true" />
    {/if}
  </button>
</div>
