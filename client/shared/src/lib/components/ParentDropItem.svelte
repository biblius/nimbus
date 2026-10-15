<script lang="ts" module>
  export const PARENT_DROPPABLE_ID = "__parent__";
</script>

<script lang="ts">
  import { FolderUp } from "@lucide/svelte";
  import { createDroppable } from "@dnd-kit/svelte";
  import { cn } from "../utils";

  let {
    columns,
    onOpen,
  }: {
    columns: string;
    onOpen: () => void;
  } = $props();

  const droppable = createDroppable({
    id: PARENT_DROPPABLE_ID,
    accept: "entry",
    disabled: false,
  });

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

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<li
  {@attach droppable.attach}
  class={cn(
    columns,
    "min-h-14 py-2 text-muted-foreground select-none hover:bg-accent/60",
    droppable.isDropTarget && "bg-primary/10 ring-1 ring-primary ring-inset",
  )}
  aria-label="Parent folder"
  ondblclick={onRowDoubleClick}
>
  <div class="flex min-h-12 w-full min-w-0 items-center gap-3 overflow-hidden">
    <button
      type="button"
      class="group relative flex size-12 shrink-0 cursor-pointer items-center justify-center overflow-hidden rounded-lg bg-muted text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      aria-label="Go to parent folder"
      title="Open"
      onclick={onIconClick}
    >
      <FolderUp size={24} strokeWidth={1.75} />
    </button>
    <div class="flex min-h-12 min-w-0 flex-1 items-center">
      <span class="truncate text-sm font-medium">..</span>
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
