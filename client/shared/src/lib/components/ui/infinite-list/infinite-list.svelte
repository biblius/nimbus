<script lang="ts" generics="T">
  import { untrack, type Snippet } from "svelte";
  import { toastError } from "../../../toast";
  import type { ListQuery, ListSearch, Page } from "../../../types";
  import { cn } from "../../../utils";
  import QuerySearch, {
    type QueryColumn,
  } from "../query-search/query-search.svelte";
  import InfiniteScroller from "./infinite-scroller.svelte";

  let {
    load,
    columns,
    loadAmount = $bindable(10),
    total = $bindable<number>(0),
    errorMessage = "Could not load.",
    class: className,
    searchClass,
    scrollerClass,
    children,
  }: {
    /**
     * Load the next set of items, determined by query.
     */
    load: (query: ListQuery) => Promise<Page<T>>;
    columns: QueryColumn[];
    /** How many items each scroll to the bottom loads. */
    loadAmount?: number;
    total?: number | null;
    errorMessage?: string;
    class?: string;
    searchClass?: string;
    scrollerClass?: string;
    children: Snippet<[T[]]>;
  } = $props();

  /**
   * Item buffer that holds already loaded items.
   */
  let items = $state<T[] | null>(null);

  /**
   * Whether item loading failed.
   */
  let failed = $state(false);

  /**
   * The page that gets loaded next. Resets on search update.
   */
  let page = 1;

  /**
   * True if the last page of items is loaded or there was an error loading.
   */
  let done = false;

  /**
   * Currently pending load.
   */
  let pending: Promise<boolean> | null = null;

  /**
   * Used to reload the infinite scroller when the search input changes.
   * When this changes, the list is reset and a new infinite scroller is
   * mounted.
   */
  let generation = $state(0);

  let search = $state<ListSearch>({
    column: untrack(() => columns[0]?.value ?? ""),
    q: "",
  });

  function onSearch() {
    generation += 1;
    items = null;
    total = null;
    failed = false;
    page = 1;
    done = false;
    pending = null;
  }

  async function loadNext(): Promise<boolean> {
    if (done) {
      return false;
    }

    if (pending) {
      return pending;
    }

    const current = page;
    const currentGeneration = generation;
    const q = search.q.trim();
    const column = search.column;

    const task = load({
      page: current,
      perPage: loadAmount,
      search: q ? { q, column } : undefined,
    })
      .then((listed) => {
        if (currentGeneration !== generation) {
          return false;
        }

        if (!items) {
          items = [];
        }

        for (const item of listed.items) {
          items.push(item);
        }

        total = listed.total;
        page = current + 1;
        done = listed.items.length === 0 || items.length >= listed.total;

        return !done;
      })
      .catch((cause) => {
        if (currentGeneration !== generation) {
          return false;
        }
        done = true;
        failed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : errorMessage,
        );
        return false;
      })
      .finally(() => {
        if (pending === task) {
          pending = null;
        }
      });

    pending = task;

    return pending;
  }
</script>

<div class={cn("space-y-1.5", className)}>
  <QuerySearch
    {columns}
    class={searchClass}
    bind:query={search}
    onUpdate={onSearch}
  />
  {#key generation}
    <InfiniteScroller {loadNext} bind:loadAmount class={scrollerClass}>
      {#if failed && !items}
        <p class="px-3 py-2 text-sm text-muted-foreground">{errorMessage}</p>
      {:else if items}
        {@render children(items)}
      {/if}
    </InfiniteScroller>
  {/key}
</div>
