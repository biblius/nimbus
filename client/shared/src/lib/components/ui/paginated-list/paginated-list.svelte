<script lang="ts" generics="T">
  import { untrack, type Snippet } from "svelte";
  import type { ListQuery, ListSearch, Page } from "../../../types";
  import type { StoredSort } from "../../../storage";
  import QuerySearch, {
    type QueryColumn,
  } from "../query-search/query-search.svelte";
  import PaginatedPages from "./paginated-pages.svelte";

  let {
    load,
    searchColumns,
    sort,
    onLoaded,
    revision = 0,
    listHeader,
    children,
  }: {
    load: (query: ListQuery) => Promise<Page<T>>;
    searchColumns: QueryColumn[];
    /** Active sort; changing it remounts the page loader. */
    sort: StoredSort;
    onLoaded?: (items: T[]) => void;
    /** Bump to remount and reload the current query. */
    revision?: number;
    listHeader?: Snippet;
    children: Snippet<[T[]]>;
  } = $props();

  let search = $state<ListSearch>({
    column: untrack(() => searchColumns[0]?.value ?? ""),
    q: "",
  });

  /**
   * Used to reload the list. When this changes, the loaded page is reset
   * and a new one is mounted.
   */
  let generation = $state(0);

  function onSearch() {
    generation += 1;
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3 [overflow-anchor:none]">
  <div class="flex shrink-0 items-center gap-2">
    <QuerySearch
      class="min-w-0 flex-1"
      columns={searchColumns}
      bind:query={search}
      onUpdate={onSearch}
    />
  </div>

  {#key `${generation}-${revision}-${sort.column}-${sort.direction}`}
    <PaginatedPages
      {load}
      {search}
      sortColumn={sort.column}
      sortDirection={sort.direction}
      {onLoaded}
      {listHeader}
      {children}
    />
  {/key}
</div>
