<script lang="ts">
  import { FolderUp } from "@lucide/svelte";
  import { createDroppable } from "@dnd-kit/svelte";
  import { PARENT_DROPPABLE_ID } from "../../file-browser/drop";
  import type { FilesViewMode } from "../../storage";
  import { cn } from "../../utils";

  let {
    columns,
    viewMode = "list",
    onOpen,
  }: {
    columns: string;
    viewMode?: FilesViewMode;
    onOpen: () => void;
  } = $props();

  const droppable = createDroppable({
    id: PARENT_DROPPABLE_ID,
    accept: "entry",
    disabled: false,
  });

  const tiles = $derived(
    viewMode === "tiles-small" || viewMode === "tiles-large",
  );
  const large = $derived(viewMode === "tiles-large");
  const compact = $derived(viewMode === "compact");
  const iconSize = $derived(tiles ? (large ? 44 : 30) : compact ? 22 : 30);
  const boxClass = $derived(compact ? "size-11" : "size-14");
  const rowPadClass = $derived(compact ? "min-h-11 py-1" : "min-h-16 py-2");
  const innerRowClass = $derived(
    compact ? "min-h-11 gap-2.5" : "min-h-14 gap-3",
  );
  const nameAreaClass = $derived(compact ? "min-h-11" : "min-h-14");
  const textClass = $derived(compact ? "text-xs" : "text-sm");

  function onIconClick(event: MouseEvent) {
    event.stopPropagation();
    onOpen();
  }

  function onRowDoubleClick(event: MouseEvent) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-no-open]")
    ) {
      return;
    }
    onOpen();
  }
</script>

{#if tiles}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <li
    {@attach droppable.attach}
    class={cn(
      "relative flex flex-col rounded-lg text-muted-foreground select-none hover:bg-accent/60",
      large ? "gap-1.5 p-2" : "gap-1 p-1.5",
      droppable.isDropTarget && "bg-primary/10 ring-1 ring-primary ring-inset",
    )}
    aria-label="Parent folder"
    ondblclick={onRowDoubleClick}
  >
    <button
      type="button"
      class="group relative flex aspect-square w-full cursor-pointer items-center justify-center overflow-hidden rounded-md bg-muted text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      aria-label="Go to parent folder"
      title="Open"
      onclick={onIconClick}
    >
      <FolderUp size={iconSize} strokeWidth={1.75} />
    </button>
    <div class="flex min-w-0 flex-col items-center">
      <span
        class={cn(
          "w-full text-center font-medium",
          large ? "text-sm" : "text-xs",
        )}
      >
        ..
      </span>
    </div>
  </li>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <li
    {@attach droppable.attach}
    class={cn(
      columns,
      rowPadClass,
      "text-muted-foreground select-none hover:bg-accent/60",
      droppable.isDropTarget && "bg-primary/10 ring-1 ring-primary ring-inset",
    )}
    aria-label="Parent folder"
    ondblclick={onRowDoubleClick}
  >
    <div
      class={cn(
        "flex w-full min-w-0 items-center overflow-hidden",
        innerRowClass,
      )}
    >
      <button
        type="button"
        class={cn(
          "group relative flex shrink-0 cursor-pointer items-center justify-center overflow-hidden rounded-lg bg-muted text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
          boxClass,
        )}
        aria-label="Go to parent folder"
        title="Open"
        onclick={onIconClick}
      >
        <FolderUp size={iconSize} strokeWidth={1.75} />
      </button>
      <div class={cn("flex min-w-0 flex-1 items-center", nameAreaClass)}>
        <span class={cn("truncate font-medium", textClass)}>..</span>
      </div>
    </div>
    <span
      class="hidden min-w-0 overflow-hidden text-right text-sm tabular-nums @lg:block"
      aria-hidden="true"
    ></span>
    <span class="hidden min-w-0 truncate text-sm @2xl:block" aria-hidden="true"
    ></span>
    <span aria-hidden="true"></span>
  </li>
{/if}
