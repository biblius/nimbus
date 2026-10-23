<script lang="ts">
  import {
    Download,
    EllipsisVertical,
    Eye,
    Folder,
    FolderInput,
    Link as LinkIcon,
    Link2Off,
    Share2,
    Trash,
  } from "@lucide/svelte";
  import { createDraggable, createDroppable } from "@dnd-kit/svelte";
  import { onDestroy, untrack } from "svelte";
  import {
    FILE_ICON_CHIP_CLASS,
    fileIconChipStyle,
    hasListThumbnail,
    isPreviewable,
  } from "../../fileKind";
  import { formatBytes } from "../../format";
  import { session } from "../../session.svelte";
  import type { FileView } from "../../types";
  import { cn, hasFinePointerHover } from "../../utils";
  import Button from "../ui/button/button.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import Menu from "../ui/menu/menu.svelte";
  import FileEntryContextMenu from "./FileEntryContextMenu.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import SharedBadge from "./SharedBadge.svelte";

  const LONG_PRESS_MS = 480;
  /** Match parent PointerSensor drag distance so long-press cancels before drag starts. */
  const MOVE_THRESHOLD_PX = 8;

  let {
    entry,
    rootId,
    name,
    size,
    selected,
    multiSelected = false,
    selectMode = false,
    canWrite = true,
    canShare = true,
    onSelect,
    onLongSelect,
    onOpen,
    onPreview,
    onDownload,
    onCopy,
    canCopy = true,
    onShare,
    onUnshare,
    onCopyLink,
    onMove,
    onDelete,
    onPrepareAction,
    thumbnailUrl,
  }: {
    entry: FileView;
    rootId: string;
    thumbnailUrl?: string;
    name: string;
    size: "small" | "large";
    selected: boolean;
    /** True when this tile is selected as part of a multi-item selection. */
    multiSelected?: boolean;
    /** When true, plain taps toggle and icon clicks do not open/preview. */
    selectMode?: boolean;
    /** False when the caller lacks WRITE on the current root. */
    canWrite?: boolean;
    /** False when the caller lacks SHARE on the current root. */
    canShare?: boolean;
    /** False when there are no writable folders to copy into. */
    canCopy?: boolean;
    onSelect: (event: MouseEvent) => void;
    /** Long-press: enter select mode with this item as the only selection. */
    onLongSelect?: () => void;
    onOpen: () => void;
    onPreview: () => void;
    onDownload: () => void;
    onCopy: () => void;
    /** Create a public link for this entry. */
    onShare: () => void | Promise<void>;
    /** Stop publicly sharing this entry. */
    onUnshare: () => void | Promise<void>;
    /** Copy the public link for an already-shared entry. */
    onCopyLink: () => void | Promise<void>;
    onMove: () => void;
    onDelete: () => void;
    /** Prepare action target (e.g. lock multi-select) before opening menus. */
    onPrepareAction?: () => void;
  } = $props();

  const file = $derived(entry.file);
  const bulk = $derived(selected && multiSelected);
  const large = $derived(size === "large");
  const iconSize = $derived(large ? 44 : 30);

  const writeDeniedTitle = "You do not have write access";
  const shareDeniedTitle = "You do not have share access";

  function copyMenuTitle(): string {
    if (canCopy) {
      return "Copy";
    }
    return "There are no writable folders to copy to";
  }

  const fileId = untrack(() => entry.file.id);
  const directory = untrack(() => entry.file.kind === "directory");
  const draggable = createDraggable({
    id: fileId,
    type: "entry",
    get disabled() {
      return !canWrite;
    },
  });
  // Files are not drop targets. A folder is the only valid parent.
  const droppable = createDroppable({
    id: fileId,
    accept: "entry",
    get disabled() {
      return !directory || !canWrite;
    },
  });

  const previewable = $derived(isPreviewable(entry));
  const showThumbnail = $derived(hasListThumbnail(entry));

  let longPressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPressOrigin: { x: number; y: number } | null = null;
  let suppressClick = false;
  let longPressFired = false;

  let menuOpen = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);

  function clearLongPressTimer() {
    if (longPressTimer !== undefined) {
      clearTimeout(longPressTimer);
      longPressTimer = undefined;
    }
    longPressOrigin = null;
  }

  onDestroy(() => {
    clearLongPressTimer();
  });

  function isActionTarget(target: EventTarget | null) {
    return (
      target instanceof Element && Boolean(target.closest("[data-no-open]"))
    );
  }

  function onTilePointerDown(event: PointerEvent) {
    // button 0 = primary (left click / touch); ignore right/middle.
    if (
      event.button !== 0 ||
      isActionTarget(event.target) ||
      !onLongSelect ||
      hasFinePointerHover()
    ) {
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
      onLongSelect();
    }, LONG_PRESS_MS);
  }

  function onTilePointerMove(event: PointerEvent) {
    if (!longPressOrigin) {
      return;
    }

    const dx = event.clientX - longPressOrigin.x;
    const dy = event.clientY - longPressOrigin.y;

    if (dx * dx + dy * dy > MOVE_THRESHOLD_PX * MOVE_THRESHOLD_PX) {
      clearLongPressTimer();
    }
  }

  function onTilePointerEnd() {
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

  function onTileClick(event: MouseEvent) {
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

  /** Icon/media: open / preview (or select only with modifiers / select mode). */
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

  function onTileDoubleClick(event: MouseEvent) {
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

  function onTileContextMenu(event: MouseEvent) {
    if (longPressFired) {
      event.preventDefault();
      longPressFired = false;
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    onPrepareAction?.();
    menuX = event.clientX;
    menuY = event.clientY;
    menuOpen = true;
  }

  let previewUrl = $state<string | null>(null);

  $effect(() => {
    const fileId = entry.file.id;
    // Both small and large tiles display well above 64 CSS px; use the 256 icon.
    const hasPreview = entry.metadata?.preview256 != null;

    previewUrl = null;

    if (entry.file.kind !== "file" || !hasPreview || !hasListThumbnail(entry)) {
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;

    session.client
      .icon(rootId, fileId, 256)
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
    "group/tile relative flex flex-col rounded-lg outline-none select-none [-webkit-touch-callout:none] hover:bg-accent/60 focus:outline-none focus-visible:outline-none",
    large ? "gap-1.5 p-2" : "gap-1 p-1.5",
    selected && "bg-accent/40",
    selected &&
      (selectMode || multiSelected) &&
      "ring-1 ring-primary/70 ring-inset",
    draggable.isDragging && "opacity-40",
    directory &&
      droppable.isDropTarget &&
      "bg-primary/10 ring-1 ring-primary ring-inset",
  )}
  onclick={onTileClick}
  ondblclick={onTileDoubleClick}
  onpointerdown={onTilePointerDown}
  onpointermove={onTilePointerMove}
  onpointerup={onTilePointerEnd}
  onpointercancel={onTilePointerEnd}
  oncontextmenu={onTileContextMenu}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class={cn(
      "absolute top-1 right-1 z-1 opacity-0 transition-opacity group-hover/tile:opacity-100 focus-within:opacity-100",
      selected && "opacity-100",
    )}
    data-no-select
    data-no-open
    onclick={(event) => event.stopPropagation()}
    ondblclick={(event) => event.stopPropagation()}
  >
    <Menu class="mt-1 w-44">
      {#snippet trigger(props)}
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="bg-background/90 text-muted-foreground shadow-sm backdrop-blur-sm"
          aria-label="Actions for {name}"
          {...props}
          onclick={(event) => {
            onPrepareAction?.();
            props.onclick?.(event);
          }}
        >
          <EllipsisVertical size={16} />
        </Button>
      {/snippet}
      <MenuItem onclick={onDownload}>
        <Download size={16} />
        {bulk ? "Download as zip" : "Download"}
      </MenuItem>
      <span class="block w-full" title={canCopy ? undefined : copyMenuTitle()}>
        <MenuItem
          disabled={!canCopy}
          aria-label={copyMenuTitle()}
          onclick={onCopy}
        >
          <Share2 size={16} />
          Copy
        </MenuItem>
      </span>
      <span
        class="block w-full"
        title={canWrite ? undefined : writeDeniedTitle}
      >
        <MenuItem
          disabled={!canWrite}
          aria-label={canWrite ? "Move" : writeDeniedTitle}
          onclick={onMove}
        >
          <FolderInput size={16} />
          Move
        </MenuItem>
      </span>
      {#if file.sharingMode}
        {#if !bulk}
          <MenuItem onclick={onCopyLink}>
            <LinkIcon size={16} />
            Copy public link
          </MenuItem>
        {/if}
        <span
          class="block w-full"
          title={canShare ? undefined : shareDeniedTitle}
        >
          <MenuItem
            disabled={!canShare}
            aria-label={canShare ? "Stop sharing" : shareDeniedTitle}
            onclick={onUnshare}
          >
            <Link2Off size={16} />
            Stop sharing
          </MenuItem>
        </span>
      {:else}
        <span
          class="block w-full"
          title={canShare ? undefined : shareDeniedTitle}
        >
          <MenuItem
            disabled={!canShare}
            aria-label={canShare ? "Create public link" : shareDeniedTitle}
            onclick={onShare}
          >
            <LinkIcon size={16} />
            Create public link
          </MenuItem>
        </span>
      {/if}
      <span
        class="block w-full"
        title={canWrite ? undefined : writeDeniedTitle}
      >
        <MenuItem
          class="text-destructive hover:text-destructive [&_svg]:text-destructive"
          disabled={!canWrite}
          aria-label={canWrite ? "Delete" : writeDeniedTitle}
          onclick={onDelete}
        >
          <Trash size={16} />
          Delete
        </MenuItem>
      </span>
    </Menu>
  </div>

  {#if file.kind === "directory"}
    <span class="relative aspect-square w-full">
      <button
        type="button"
        class="group flex size-full cursor-pointer items-center justify-center overflow-hidden rounded-md bg-primary/10 text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        data-no-select
        aria-label="Open {name}"
        title="Open"
        onclick={onIconClick}
      >
        <Folder size={iconSize} strokeWidth={1.75} />
      </button>
      {#if file.sharingMode}
        <SharedBadge
          size={large ? "lg" : "md"}
          expiresAt={file.shareExpiresAt}
        />
      {/if}
    </span>
  {:else if previewable}
    <span class="relative aspect-square w-full">
      <button
        type="button"
        class="group relative size-full cursor-pointer overflow-hidden rounded-md bg-muted ring-1 ring-border/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        data-no-select
        aria-label="Preview {name}"
        title="Preview"
        onclick={onIconClick}
      >
        {#if showThumbnail && (thumbnailUrl ?? previewUrl)}
          <img
            src={thumbnailUrl ?? previewUrl}
            alt=""
            class="size-full object-cover"
          />
        {:else}
          <span
            class={cn(
              "flex size-full items-center justify-center",
              FILE_ICON_CHIP_CLASS,
            )}
            style={fileIconChipStyle(file.fileType)}
          >
            <FileTypeIcon fileType={file.fileType} size={iconSize} />
          </span>
        {/if}
        <span
          class="pointer-events-none absolute inset-0 flex items-center justify-center rounded-md bg-background/80 text-foreground opacity-0 transition-opacity group-hover:opacity-100"
          aria-hidden="true"
        >
          <Eye size={large ? 22 : 18} strokeWidth={2} />
        </span>
      </button>
      {#if file.sharingMode}
        <SharedBadge
          size={large ? "lg" : "md"}
          expiresAt={file.shareExpiresAt}
        />
      {/if}
    </span>
  {:else}
    <span
      class={cn(
        "relative flex aspect-square w-full items-center justify-center rounded-md ring-1",
        FILE_ICON_CHIP_CLASS,
      )}
      style={fileIconChipStyle(file.fileType)}
    >
      <FileTypeIcon fileType={file.fileType} size={iconSize} />
      {#if file.sharingMode}
        <SharedBadge
          size={large ? "lg" : "md"}
          expiresAt={file.shareExpiresAt}
        />
      {/if}
    </span>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    {@attach draggable.attachHandle}
    class="flex min-w-0 flex-col items-center outline-none focus:outline-none focus-visible:outline-none"
    data-no-select
    title={name}
    onclick={onNameClick}
  >
    <span
      class={cn(
        "w-full text-center font-medium",
        large ? "line-clamp-2 text-sm" : "truncate text-xs",
      )}
    >
      {name}
    </span>
    {#if file.kind === "file"}
      <span class="text-[11px] text-muted-foreground tabular-nums">
        {formatBytes(file.sizeBytes)}
      </span>
    {/if}
  </div>
</li>

<FileEntryContextMenu
  bind:open={menuOpen}
  x={menuX}
  y={menuY}
  {bulk}
  shared={Boolean(file.sharingMode)}
  {canWrite}
  {canShare}
  {canCopy}
  {onDownload}
  {onCopy}
  {onShare}
  {onUnshare}
  {onCopyLink}
  {onMove}
  {onDelete}
/>
