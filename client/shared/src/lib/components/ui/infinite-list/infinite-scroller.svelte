<script lang="ts">
  import { onMount, tick, type Snippet } from "svelte";
  import { cn } from "../../../utils";

  let {
    loadNext,
    loadAmount = $bindable(10),
    class: className,
    children,
  }: {
    /**
     * Loads the next page.
     * Resolves `true` when another page exists.
     */
    loadNext: () => Promise<boolean>;
    /**
     * How many items each scroll to the bottom loads.
     * The first page uses the same amount.
     */
    loadAmount?: number;
    class?: string;
    children: Snippet;
  } = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let sentinel = $state<HTMLDivElement | null>(null);

  /**
   * Indicates whether the next set of items in this list is loading.
   */
  let loading = $state(false);

  /**
   * Controls whether the list has loaded all items and there is nothing
   * to load anymore. Gets set to true on errors.
   */
  let done = false;

  async function load() {
    if (loading || done) {
      return;
    }

    loading = true;

    try {
      while (!done) {
        const hasNext = await loadNext();

        // If there's more items, we are not done.
        done = !hasNext;

        if (done) {
          break;
        }

        // Await a tick so the children get populated
        // and move the sentinel.
        await tick();

        const box = scroller!.getBoundingClientRect();
        const mark = sentinel!.getBoundingClientRect();

        // The sentinel is below the visible box, so scrolling can load the rest.
        if (mark.top > box.bottom + 1) {
          break;
        }
      }
    } catch {
      done = true;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          load();
        }
      },
      { root: scroller },
    );

    observer.observe(sentinel!);

    load();

    return () => observer.disconnect();
  });
</script>

<div
  bind:this={scroller}
  class={cn(
    "max-h-64 overflow-y-auto overscroll-contain rounded-lg border bg-card",
    className,
  )}
  aria-busy={loading}
>
  {@render children()}
  <div bind:this={sentinel} class="h-px" aria-hidden="true"></div>
  {#if loading}
    <p class="px-3 py-2 text-sm text-muted-foreground">Loading…</p>
  {/if}
</div>
