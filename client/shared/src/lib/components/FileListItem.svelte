<script lang="ts">
  import { Download, Eye, Folder, Share2, Trash } from "@lucide/svelte";
  import { createDraggable, createDroppable } from "@dnd-kit/svelte";
  import { onDestroy, untrack } from "svelte";
  import { hasListThumbnail, isAudioFile, isPreviewable } from "../fileKind";
  import { formatBytes, formatDatetime } from "../format";
  import { session } from "../session.svelte";
  import type { FileView } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";
  import MenuItem from "./ui/menu/menu-item.svelte";
  import Menu from "./ui/menu/menu.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";

  const ICON = 24;
  const LONG_PRESS_MS = 480;
  /** Match parent PointerSensor drag distance so long-press cancels before drag starts. */
  const MOVE_THRESHOLD_PX = 8;

  let {
    entry,
    rootId,
    name,
    columns,
    selected,
    multiSelected = false,
    selectMode = false,
    onSelect,
    onLongSelect,
    onOpen,
    onPreview,
    onDownload,
    onCopy,
    onDelete,
    thumbnailUrl,
  }: {
    entry: FileView;
    rootId: string;
    thumbnailUrl?: string;
    name: string;
    columns: string;
    selected: boolean;
    /** True when this row is selected as part of a multi-item selection. */
    multiSelected?: boolean;
    /** When true, plain taps toggle and icon clicks do not open/preview. */
    selectMode?: boolean;
    onSelect: (event: MouseEvent) => void;
    /** Long-press: enter select mode with this item as the only selection. */
    onLongSelect?: () => void;
    onOpen: () => void;
    onPreview: () => void;
    onDownload: () => void;
    onCopy: () => void;
    onDelete: () => void;
  } = $props();

  const file = $derived(entry.file);

  const fileId = untrack(() => entry.file.id);
  const directory = untrack(() => entry.file.kind === "directory");
  const draggable = createDraggable({ id: fileId, type: "entry" });
  // Files are not drop targets. A folder is the only valid parent.
  const droppable = createDroppable({
    id: fileId,
    accept: "entry",
    disabled: !directory,
  });

  const previewable = $derived(isPreviewable(entry));
  const audioFile = $derived(isAudioFile(entry));
  const showThumbnail = $derived(hasListThumbnail(entry));

  let longPressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPressOrigin: { x: number; y: number } | null = null;
  let suppressClick = false;
  let longPressFired = false;

  function clearLongPressTimer() {
    if (longPressTimer !== undefined) {
      clearTimeout(longPressTimer);
      longPressTimer = undefined;
    }
    longPressOrigin = null;
  }

  onDestroy(clearLongPressTimer);

  function isActionTarget(target: EventTarget | null) {
    return (
      target instanceof Element && Boolean(target.closest("[data-no-open]"))
    );
  }

  function onRowPointerDown(event: PointerEvent) {
    if (event.button !== 0 || isActionTarget(event.target)) {
      return;
    }

    clearLongPressTimer();
    longPressFired = false;
    longPressOrigin = { x: event.clientX, y: event.clientY };

    longPressTimer = setTimeout(() => {
      longPressTimer = undefined;
      longPressOrigin = null;
      longPressFired = true;
      suppressClick = true;
      onLongSelect?.();
    }, LONG_PRESS_MS);
  }

  function onRowPointerMove(event: PointerEvent) {
    if (!longPressOrigin) {
      return;
    }

    const dx = event.clientX - longPressOrigin.x;
    const dy = event.clientY - longPressOrigin.y;

    if (dx * dx + dy * dy > MOVE_THRESHOLD_PX * MOVE_THRESHOLD_PX) {
      clearLongPressTimer();
    }
  }

  function onRowPointerEnd() {
    clearLongPressTimer();
  }

  function consumeSuppressedClick(event: MouseEvent) {
    if (!suppressClick) {
      return false;
    }

    suppressClick = false;

    event.preventDefault();
    event.stopPropagation();

    return true;
  }

  function onRowClick(event: MouseEvent) {
    if (consumeSuppressedClick(event)) {
      return;
    }

    if (
      event.target instanceof Element &&
      event.target.closest("button, a, input, [data-no-select]")
    ) {
      return;
    }

    onSelect(event);
  }

  /** Icon: open / preview (or select only with modifiers / select mode). */
  function onIconClick(event: MouseEvent) {
    event.stopPropagation();

    if (consumeSuppressedClick(event)) {
      return;
    }

    if (
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      selectMode ||
      multiSelected
    ) {
      event.preventDefault();
      onSelect(event);
      return;
    }

    onSelect(event);

    if (file.kind === "directory") {
      onOpen();
      return;
    }

    if (previewable) {
      onPreview();
    }
  }

  /** Name: select + drag handle (does not open). */
  function onNameClick(event: MouseEvent) {
    event.stopPropagation();

    if (consumeSuppressedClick(event)) {
      return;
    }

    onSelect(event);
  }

  function onRowDoubleClick(event: MouseEvent) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-no-open]")
    ) {
      return;
    }

    if (file.kind === "directory") {
      onOpen();
      return;
    }

    if (previewable) {
      onPreview();
    }
  }

  function onRowContextMenu(event: MouseEvent) {
    if (longPressFired) {
      event.preventDefault();
      longPressFired = false;
    }
  }

  let previewUrl = $state<string | null>(null);

  $effect(() => {
    const fileId = entry.file.id;
    const hasPreview = entry.metadata?.preview != null;

    previewUrl = null;

    if (
      entry.file.kind !== "file" ||
      !hasPreview ||
      !hasListThumbnail(entry)
    ) {
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;

    session.client
      .preview(rootId, fileId)
      .then((blob) => {
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(blob);
        previewUrl = objectUrl;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<li
  {@attach draggable.attach}
  {@attach directory ? droppable.attach : undefined}
  class={cn(
    columns,
    "min-h-14 py-2 outline-none select-none [-webkit-touch-callout:none] hover:bg-accent/60 focus:outline-none focus-visible:outline-none",
    selected && "bg-accent/40",
    selected &&
      (selectMode || multiSelected) &&
      "ring-1 ring-primary/70 ring-inset",
    draggable.isDragging && "opacity-40",
    directory &&
      droppable.isDropTarget &&
      "bg-primary/10 ring-1 ring-primary ring-inset",
  )}
  onclick={onRowClick}
  ondblclick={onRowDoubleClick}
  onpointerdown={onRowPointerDown}
  onpointermove={onRowPointerMove}
  onpointerup={onRowPointerEnd}
  onpointercancel={onRowPointerEnd}
  oncontextmenu={onRowContextMenu}
>
  <div class="flex min-h-12 w-full min-w-0 items-center gap-3 overflow-hidden">
    {#if file.kind === "directory"}
      <button
        type="button"
        class="group relative flex size-12 shrink-0 cursor-pointer items-center justify-center overflow-hidden rounded-lg bg-primary/10 text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        data-no-select
        aria-label="Open {name}"
        title="Open"
        onclick={onIconClick}
      >
        <Folder size={ICON} strokeWidth={1.75} />
      </button>
    {:else if previewable}
      <button
        type="button"
        class="group relative size-12 shrink-0 cursor-pointer overflow-hidden rounded-lg ring-1 ring-border/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        data-no-select
        aria-label="Preview {name}"
        title="Preview"
        onclick={onIconClick}
      >
        {#if showThumbnail && (thumbnailUrl ?? previewUrl)}
          <img
            src={thumbnailUrl ?? previewUrl}
            alt=""
            width="48"
            height="48"
            class="size-full object-cover"
          />
        {:else}
          <span
            class="flex size-full items-center justify-center bg-muted text-muted-foreground"
          >
            <FileTypeIcon fileType={file.fileType} size={ICON} />
          </span>
        {/if}
        <span
          class="pointer-events-none absolute inset-0 flex items-center justify-center rounded-lg bg-background/80 text-foreground opacity-0 transition-opacity group-hover:opacity-100"
          aria-hidden="true"
        >
          <Eye size={20} strokeWidth={2} />
        </span>
      </button>
    {:else}
      <span
        class="flex size-12 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground ring-1 ring-border/60"
      >
        <FileTypeIcon fileType={file.fileType} size={ICON} />
      </span>
    {/if}

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      {@attach draggable.attachHandle}
      class="flex min-h-12 min-w-0 flex-1 items-center outline-none focus:outline-none focus-visible:outline-none"
      data-no-select
      title={name}
      onclick={onNameClick}
    >
      <span class="truncate text-sm font-medium">{name}</span>
    </div>
  </div>

  <span
    class="hidden min-w-0 overflow-hidden text-right text-sm text-muted-foreground tabular-nums @lg:block"
  >
    {file.kind === "directory" ? "—" : formatBytes(file.sizeBytes)}
  </span>
  <span
    class="hidden min-w-0 truncate text-sm text-muted-foreground @2xl:block"
  >
    {formatDatetime(file.createdAt)}
  </span>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex items-center justify-end gap-1"
    data-no-select
    data-no-open
    onclick={(event) => event.stopPropagation()}
    ondblclick={(event) => event.stopPropagation()}
  >
    {#if file.kind === "file"}
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground"
        aria-label="Download {name}"
        onclick={onDownload}
      >
        <Download size={16} />
      </Button>
    {/if}
    <Menu class="mt-1">
      {#snippet trigger(props)}
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="text-muted-foreground"
          aria-label="Share {name}"
          {...props}
        >
          <Share2 size={16} />
        </Button>
      {/snippet}
      <MenuItem onclick={onCopy}>Copy to…</MenuItem>
    </Menu>
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      class="text-muted-foreground hover:text-destructive"
      aria-label="Delete {name}"
      onclick={onDelete}
    >
      <Trash size={16} />
    </Button>
  </div>
</li>
