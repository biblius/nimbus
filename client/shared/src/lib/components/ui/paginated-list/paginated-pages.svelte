<script lang="ts" generics="T">
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
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
    listHeader,
    children,
  }: {
    load: (query: ListQuery) => Promise<Page<T>>;
    search: ListSearch;
    sortColumn: string;
    sortDirection: "asc" | "desc";
    onLoaded?: (items: T[]) => void;
    /** Optional column header above the scrollable rows. */
    listHeader?: Snippet;
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

<!-- Content-height card (not flex-1 fill). Page scroll lives on the parent. -->
<div
  class="@container flex min-w-0 flex-col overflow-hidden rounded-lg border bg-card"
>
  <div
    class="flex shrink-0 items-center justify-between gap-3 border-b bg-muted/40 px-3 py-1.5"
  >
    <p class="text-xs text-muted-foreground tabular-nums sm:text-sm">
      Page {page} of {pages}
      <span class="text-muted-foreground/80">· {total} total</span>
    </p>
    <div class="flex items-center gap-1.5">
      <Button
        type="button"
        variant="outline"
        size="sm"
        class="size-8 border-border bg-background p-0 shadow-sm"
        aria-label="Previous page"
        disabled={loading || page <= 1}
        onclick={() => loadPage(page - 1)}
      >
        <ChevronLeft size={16} />
      </Button>
      <Button
        type="button"
        variant="outline"
        size="sm"
        class="size-8 border-border bg-background p-0 shadow-sm"
        aria-label="Next page"
        disabled={loading || page >= pages}
        onclick={() => loadPage(page + 1)}
      >
        <ChevronRight size={16} />
      </Button>
    </div>
  </div>

  {#if items && items.length > 0 && listHeader}
    <div class="shrink-0 border-b">
      {@render listHeader()}
    </div>
  {/if}

  <div>
    {#if failed}
      <p class="px-4 py-8 text-center text-sm text-muted-foreground">
        Could not load this page.
      </p>
    {:else if !items}
      <p class="px-4 py-8 text-center text-sm text-muted-foreground">
        Loading…
      </p>
    {:else}
      {@render children(items)}
    {/if}
  </div>
</div>
