<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { cn } from "../../../utils";

  let {
    items,
    itemHeight,
    columns = 1,
    gap = 0,
    overscan = 8,
    getKey,
    class: className,
    windowClass,
    as = "div",
    element = $bindable(),
    header,
    leading,
    children,
  }: {
    items: T[];
    /** Row stride in px (cell height; gaps are added via `gap`). */
    itemHeight: number;
    columns?: number;
    /** Gap between grid cells in px (tiles). */
    gap?: number;
    overscan?: number;
    getKey?: (item: T, index: number) => string | number;
    class?: string;
    /** Classes on the positioned window that holds visible items. */
    windowClass?: string;
    /** Element type for the window (`ul` when children are `li`). */
    as?: "div" | "ul";
    element?: HTMLElement | undefined;
    header?: Snippet;
    /** Optional first cell/row before `items` (e.g. parent `..` tile). */
    leading?: Snippet;
    children: Snippet<[T, number]>;
  } = $props();

  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let headerHeight = $state(0);

  const cols = $derived(Math.max(1, Math.floor(columns)));
  const rowGap = $derived(Math.max(0, gap));
  const rowStride = $derived(itemHeight + rowGap);
  const leadingCount = $derived(leading ? 1 : 0);
  const totalCount = $derived(items.length + leadingCount);
  const rowCount = $derived(
    totalCount === 0 ? 0 : Math.ceil(totalCount / cols),
  );
  const totalHeight = $derived(
    rowCount === 0
      ? 0
      : rowCount * itemHeight + Math.max(0, rowCount - 1) * rowGap,
  );

  const endRow = $derived.by(() => {
    if (rowCount === 0 || rowStride <= 0) {
      return -1;
    }
    return Math.min(
      rowCount - 1,
      Math.ceil((scrollTop - headerHeight + viewportHeight) / rowStride) +
        overscan,
    );
  });

  const startRow = $derived.by(() => {
    if (rowCount === 0 || rowStride <= 0) {
      return 0;
    }
    const raw = Math.max(
      0,
      Math.floor((scrollTop - headerHeight) / rowStride) - overscan,
    );
    // Keep a non-empty window when scrollTop is past the new content
    // (e.g. after navigating from a long folder into a short one).
    return Math.min(raw, Math.max(0, endRow));
  });

  const offsetY = $derived(startRow * rowStride);

  type VisibleSlot =
    | { kind: "leading"; key: string }
    | { kind: "item"; item: T; index: number; key: string | number };

  const visible = $derived.by(() => {
    const out: VisibleSlot[] = [];

    for (let row = startRow; row <= endRow; row++) {
      for (let col = 0; col < cols; col++) {
        const slot = row * cols + col;

        if (slot >= totalCount) {
          break;
        }

        if (leading && slot === 0) {
          out.push({ kind: "leading", key: "__leading__" });
          continue;
        }

        const index = slot - leadingCount;
        const item = items[index];
        out.push({
          kind: "item",
          item,
          index,
          key: getKey?.(item, index) ?? index,
        });
      }
    }

    return out;
  });

  const observeRoot: Attachment<HTMLElement> = (node) => {
    const onScroll = () => {
      scrollTop = node.scrollTop;
    };

    const observer = new ResizeObserver(() => {
      viewportHeight = node.clientHeight;
    });

    node.addEventListener("scroll", onScroll, { passive: true });

    observer.observe(node);
    scrollTop = node.scrollTop;
    viewportHeight = node.clientHeight;

    return () => {
      node.removeEventListener("scroll", onScroll);
      observer.disconnect();
    };
  };

  const observeHeader: Attachment<HTMLElement> = (node) => {
    const observer = new ResizeObserver(() => {
      headerHeight = node.offsetHeight;
    });

    observer.observe(node);
    headerHeight = node.offsetHeight;

    return () => {
      observer.disconnect();
      headerHeight = 0;
    };
  };
</script>

<div
  bind:this={element}
  class={cn("min-h-0 flex-1 overflow-y-auto overscroll-contain", className)}
  {@attach observeRoot}
>
  {#if header}
    <div {@attach observeHeader}>
      {@render header()}
    </div>
  {/if}

  <div class="relative w-full" style:height="{totalHeight}px">
    <svelte:element
      this={as}
      class={cn(
        "absolute top-0 right-0 left-0",
        cols > 1 && "grid",
        windowClass,
      )}
      style:transform="translateY({offsetY}px)"
      style:grid-template-columns={cols > 1
        ? `repeat(${cols}, minmax(0, 1fr))`
        : undefined}
      style:gap={cols > 1 && rowGap > 0 ? `${rowGap}px` : undefined}
    >
      {#each visible as slot (slot.key)}
        {#if slot.kind === "leading"}
          {@render leading?.()}
        {:else}
          {@render children(slot.item, slot.index)}
        {/if}
      {/each}
    </svelte:element>
  </div>
</div>
