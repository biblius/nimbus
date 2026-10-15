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
</script>

<li
  {@attach droppable.attach}
  class={cn(
    columns,
    "min-h-14 py-2 text-muted-foreground",
    droppable.isDropTarget && "bg-primary/10 ring-1 ring-primary ring-inset",
  )}
  aria-label="Parent folder"
>
  <button
    type="button"
    class="flex min-h-12 w-full min-w-0 items-center gap-3 overflow-hidden text-left text-sm font-medium"
    onclick={onOpen}
  >
    <span
      class="flex size-12 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
    >
      <FolderUp size={24} strokeWidth={1.75} />
    </span>
    <span class="truncate">..</span>
  </button>
  <span
    class="hidden min-w-0 overflow-hidden text-right text-sm tabular-nums @lg:block"
    aria-hidden="true"
  ></span>
  <span
    class="hidden min-w-0 truncate text-sm @2xl:block"
    aria-hidden="true"
  ></span>
  <span aria-hidden="true"></span>
</li>
