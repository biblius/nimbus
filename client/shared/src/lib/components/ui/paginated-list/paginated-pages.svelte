<script lang="ts" generics="T">
  import { onMount, type Snippet } from "svelte";
  import { toastError } from "../../../toast";
  import type { ListQuery, ListSearch, Page } from "../../../types";
  import Button from "../button/button.svelte";

  let {
    load,
    search,
    sortColumn,
    sortDirection,
    onLoaded: onPageLoad,
    children,
  }: {
    load: (query: ListQuery) => Promise<Page<T>>;
    search: ListSearch;
    sortColumn: string;
    sortDirection: "asc" | "desc";
    onLoaded?: (items: T[]) => void;
    children: Snippet<[T[]]>;
  } = $props();

  let items = $state<T[] | null>(null);

  let total = $state(0);

  let page = $state(1);

  let failed = $state(false);

  let loading = $state(false);

  let generation = 0;

  const perPage = 10;

  const pages = $derived(Math.max(1, Math.ceil(total / perPage)));

  async function loadPage(nextPage: number) {
    const current = ++generation;

    loading = true;
    failed = false;

    try {
      const q = search.q.trim();
      const result = await load({
        page: nextPage,
        perPage,
        search: q ? { q, column: search.column } : undefined,
        sort: { column: sortColumn, direction: sortDirection },
      });

      if (current !== generation) {
        return;
      }

      items = result.items;
      total = result.total;

      page = nextPage;

      onPageLoad?.(result.items);
    } catch (cause) {
      if (current !== generation) {
        return;
      }
      failed = true;
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load this page",
      );
    } finally {
      if (current === generation) {
        loading = false;
      }
    }
  }

  onMount(() => {
    loadPage(1);
  });
</script>

<div class="min-h-130">
  {#if failed}
    <p class="text-sm text-muted-foreground">Could not load this page.</p>
  {:else if !items}
    <p class="text-sm text-muted-foreground">Loading…</p>
  {:else}
    {@render children(items)}
  {/if}
</div>

<div class="ml-auto grid w-fit grid-cols-[2rem_8rem_2rem] items-center gap-2">
  <Button
    type="button"
    variant="outline"
    size="sm"
    class="size-8 p-0"
    aria-label="Previous page"
    disabled={loading || page <= 1}
    onclick={() => loadPage(page - 1)}
  >
    {"<"}
  </Button>
  <p class="text-center text-sm text-muted-foreground tabular-nums">
    {page}/{pages} ({total} total)
  </p>
  <Button
    type="button"
    variant="outline"
    size="sm"
    class="size-8 p-0"
    aria-label="Next page"
    disabled={loading || page >= pages}
    onclick={() => loadPage(page + 1)}
  >
    {">"}
  </Button>
</div>
