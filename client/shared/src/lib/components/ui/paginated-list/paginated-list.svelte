<script lang="ts" generics="T">
  import { ArrowUp } from "@lucide/svelte";
  import { untrack, type Snippet } from "svelte";
  import type { ListQuery, ListSearch, Page } from "../../../types";
  import QuerySearch, {
    type QueryColumn,
  } from "../query-search/query-search.svelte";
  import PaginatedPages from "./paginated-pages.svelte";

  let {
    load,
    searchColumns,
    sortColumns,
    onLoaded,
    children,
  }: {
    load: (query: ListQuery) => Promise<Page<T>>;
    searchColumns: QueryColumn[];
    sortColumns: QueryColumn[];
    onLoaded?: (items: T[]) => void;
    children: Snippet<[T[]]>;
  } = $props();

  let search = $state<ListSearch>({
    column: untrack(() => searchColumns[0]?.value ?? ""),
    q: "",
  });

  let sortColumn = $state(untrack(() => sortColumns[0]?.value ?? ""));

  let sortDirection = $state<"asc" | "desc">("asc");

  /**
   * Used to reload the list. When this changes, the loaded page is reset
   * and a new one is mounted.
   */
  let generation = $state(0);

  function onSearch() {
    generation += 1;
  }

  function toggleDirection() {
    if (sortDirection === "asc") {
      sortDirection = "desc";
    } else {
      sortDirection = "asc";
    }
    onSearch();
  }
</script>

<div class="border-bg space-y-3 [overflow-anchor:none]">
  <div class="flex items-center gap-2">
    <QuerySearch
      class="min-w-0 flex-1"
      columns={searchColumns}
      bind:query={search}
      onUpdate={onSearch}
    />
    <div class="flex shrink-0 items-center text-sm">
      <select
        class="h-9 appearance-none rounded-l-md rounded-r-none border border-input bg-field px-2 text-sm shadow-sm outline-none focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring"
        bind:value={sortColumn}
        onchange={onSearch}
        title="Sort by"
        aria-label="Sort by"
      >
        {#each sortColumns as option (option.value)}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
      <button
        type="button"
        class="inline-flex size-9 items-center justify-center rounded-l-none rounded-r-md border border-l-0 border-input bg-field text-muted-foreground shadow-sm transition-colors outline-none hover:bg-accent hover:text-foreground focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={sortDirection === "asc"
          ? "Sort ascending"
          : "Sort descending"}
        title={sortDirection === "asc" ? "Ascending" : "Descending"}
        onclick={toggleDirection}
      >
        <ArrowUp
          size={16}
          class="transition-transform duration-200 {sortDirection === 'desc'
            ? 'rotate-180'
            : ''}"
        />
      </button>
    </div>
  </div>

  {#key generation}
    <PaginatedPages
      {load}
      {search}
      {sortColumn}
      {sortDirection}
      {onLoaded}
      {children}
    />
  {/key}
</div>
