<script lang="ts">
  import {
    ChevronRight,
    Download,
    Eye,
    Folder,
    FolderInput,
    Link as LinkIcon,
    Link2Off,
    Pencil,
    Share2,
    Trash,
  } from "@lucide/svelte";
  import { createDraggable, createDroppable } from "@dnd-kit/svelte";
  import { onDestroy, tick, untrack } from "svelte";
  import {
    FILE_ICON_CHIP_CLASS,
    fileIconChipStyle,
    hasListThumbnail,
    isPreviewable,
  } from "../../fileKind";
  import { formatBytes, formatDatetime } from "../../format";
  import { session } from "../../session.svelte";
  import type { FileView } from "../../types";
  import { cn, hasFinePointerHover } from "../../utils";
  import Button from "../ui/button/button.svelte";
  import Input from "../ui/input/input.svelte";
  import ContextMenu from "../ui/menu/context-menu.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import MenuSeparator from "../ui/menu/menu-separator.svelte";
  import Menu from "../ui/menu/menu.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import SharedBadge from "./SharedBadge.svelte";

  const LONG_PRESS_MS = 480;
  /** Match parent PointerSensor drag distance so long-press cancels before drag starts. */
  const MOVE_THRESHOLD_PX = 8;

  let {
    entry,
    rootId,
    name,
    columns,
    density = "default",
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
    onRename,
    onDelete,
    onPrepareAction,
    thumbnailUrl,
  }: {
    entry: FileView;
    rootId: string;
    thumbnailUrl?: string;
    name: string;
    columns: string;
    /** Compact rows for denser Nextcloud-style list view. */
    density?: "default" | "compact";
    selected: boolean;
    /** True when this row is selected as part of a multi-item selection. */
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
    onRename: (name: string) => void | Promise<void>;
    onDelete: () => void;
    /** Prepare action target (e.g. lock multi-select) before opening menus. */
    onPrepareAction?: () => void;
  } = $props();

  const file = $derived(entry.file);
  const bulk = $derived(selected && multiSelected);
  const compact = $derived(density === "compact");
  const iconSize = $derived(compact ? 22 : 30);
  const eyeSize = $derived(compact ? 18 : 22);
  const thumbPx = $derived(compact ? 44 : 56);
  const boxClass = $derived(compact ? "size-11" : "size-14");
  const rowPadClass = $derived(compact ? "min-h-11 py-1" : "min-h-16 py-2");
  const innerRowClass = $derived(
    compact ? "min-h-11 gap-2.5" : "min-h-14 gap-3",
  );
  const nameAreaClass = $derived(compact ? "min-h-11" : "min-h-14");
  const textClass = $derived(compact ? "text-xs" : "text-sm");

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
  let shareOpen = $state(false);
  let shareCloseTimer: ReturnType<typeof setTimeout> | undefined;

  let renaming = $state(false);
  let renameValue = $state("");
  let renameError = $state("");
  let renameSaving = $state(false);
  let renameInput = $state<HTMLInputElement | undefined>();

  function clearLongPressTimer() {
    if (longPressTimer !== undefined) {
      clearTimeout(longPressTimer);
      longPressTimer = undefined;
    }
    longPressOrigin = null;
  }

  function clearShareCloseTimer() {
    if (shareCloseTimer !== undefined) {
      clearTimeout(shareCloseTimer);
      shareCloseTimer = undefined;
    }
  }

  function scheduleShareClose() {
    clearShareCloseTimer();
    shareCloseTimer = setTimeout(() => {
      shareCloseTimer = undefined;
      shareOpen = false;
    }, 120);
  }

  // Reset flyout when the context menu closes (not derivable from menuOpen alone).
  $effect(() => {
    if (!menuOpen) {
      clearShareCloseTimer();
      shareOpen = false;
    }
  });

  onDestroy(() => {
    clearLongPressTimer();
    clearShareCloseTimer();
  });

  function isActionTarget(target: EventTarget | null) {
    return (
      target instanceof Element && Boolean(target.closest("[data-no-open]"))
    );
  }

  function onRowPointerDown(event: PointerEvent) {
    // button 0 = primary (left click / touch); ignore right/middle.
    if (
      event.button !== 0 ||
      isActionTarget(event.target) ||
      renaming ||
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
    if (consumeSuppressedClick(event) || renaming) {
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

    if (consumeSuppressedClick(event) || renaming) {
      return;
    }

    onSelect(event);
  }

  function onRowDoubleClick(event: MouseEvent) {
    if (renaming) {
      return;
    }

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
      return;
    }

    if (renaming) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    onPrepareAction?.();
    shareOpen = false;
    menuX = event.clientX;
    menuY = event.clientY;
    menuOpen = true;
  }

  async function beginRename() {
    if (!canWrite) {
      return;
    }

    menuOpen = false;
    shareOpen = false;
    renameValue = file.name;
    renameError = "";
    renaming = true;
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }

  async function commitRename() {
    if (renameSaving) {
      return;
    }

    const trimmed = renameValue.trim();
    if (!trimmed) {
      renameError = "Name is required";
      return;
    }

    renameSaving = true;
    renameError = "";

    try {
      await onRename(trimmed);
      renaming = false;
    } catch {
      // Parent toasts; keep the input open for another try.
    } finally {
      renameSaving = false;
    }
  }

  function cancelRename() {
    renaming = false;
    renameError = "";
    renameSaving = false;
  }

  function onRenameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      commitRename();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancelRename();
    }
  }

  let previewUrl = $state<string | null>(null);

  $effect(() => {
    const fileId = entry.file.id;
    const hasPreview = entry.metadata?.preview64 != null;

    previewUrl = null;

    if (entry.file.kind !== "file" || !hasPreview || !hasListThumbnail(entry)) {
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;

    session.client
      .icon(rootId, fileId, 64)
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
    rowPadClass,
    "outline-none select-none [-webkit-touch-callout:none] hover:bg-accent/60 focus:outline-none focus-visible:outline-none",
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
  <div class={cn("flex w-full min-w-0 items-center", innerRowClass)}>
    {#if file.kind === "directory"}
      <span class={cn("relative shrink-0", boxClass)}>
        <button
          type="button"
          class="group flex size-full cursor-pointer items-center justify-center overflow-hidden rounded-lg bg-primary/10 text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          data-no-select
          aria-label="Open {name}"
          title="Open"
          onclick={onIconClick}
        >
          <Folder size={iconSize} strokeWidth={1.75} />
        </button>
        {#if file.sharingMode}
          <SharedBadge
            size={compact ? "sm" : "md"}
            expiresAt={file.shareExpiresAt}
          />
        {/if}
      </span>
    {:else if previewable}
      <span class={cn("relative shrink-0", boxClass)}>
        <button
          type="button"
          class="group relative size-full cursor-pointer overflow-hidden rounded-lg ring-1 ring-border/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          data-no-select
          aria-label="Preview {name}"
          title="Preview"
          onclick={onIconClick}
        >
          {#if showThumbnail && (thumbnailUrl ?? previewUrl)}
            <img
              src={thumbnailUrl ?? previewUrl}
              alt=""
              width={thumbPx}
              height={thumbPx}
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
            class="pointer-events-none absolute inset-0 flex items-center justify-center rounded-lg bg-background/80 text-foreground opacity-0 transition-opacity group-hover:opacity-100"
            aria-hidden="true"
          >
            <Eye size={eyeSize} strokeWidth={2} />
          </span>
        </button>
        {#if file.sharingMode}
          <SharedBadge
            size={compact ? "sm" : "md"}
            expiresAt={file.shareExpiresAt}
          />
        {/if}
      </span>
    {:else}
      <span
        class={cn(
          "relative flex shrink-0 items-center justify-center rounded-lg ring-1",
          FILE_ICON_CHIP_CLASS,
          boxClass,
        )}
        style={fileIconChipStyle(file.fileType)}
      >
        <FileTypeIcon fileType={file.fileType} size={iconSize} />
        {#if file.sharingMode}
          <SharedBadge
            size={compact ? "sm" : "md"}
            expiresAt={file.shareExpiresAt}
          />
        {/if}
      </span>
    {/if}

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      {@attach renaming ? undefined : draggable.attachHandle}
      class={cn(
        "flex min-w-0 flex-1 items-center outline-none focus:outline-none focus-visible:outline-none",
        nameAreaClass,
        renaming && "flex-col items-stretch justify-center gap-0.5",
      )}
      data-no-select
      data-no-drag={renaming ? "" : undefined}
      data-no-open={renaming ? "" : undefined}
      title={renaming ? undefined : name}
      onclick={onNameClick}
    >
      {#if renaming}
        <Input
          bind:ref={renameInput}
          bind:value={renameValue}
          class={cn("h-8 min-w-0 px-2", textClass)}
          aria-label="Rename {file.name}"
          aria-invalid={renameError ? true : undefined}
          autocomplete="off"
          spellcheck="false"
          disabled={renameSaving}
          data-no-drag
          data-no-select
          data-no-open
          oninput={() => (renameError = "")}
          onkeydown={onRenameKeydown}
          onclick={(event) => event.stopPropagation()}
        />
        {#if renameError}
          <p class="text-xs text-destructive">{renameError}</p>
        {/if}
      {:else}
        <span class={cn("truncate font-medium", textClass)}>{name}</span>
      {/if}
    </div>
  </div>

  <span
    class={cn(
      "hidden min-w-0 overflow-hidden text-right text-muted-foreground tabular-nums @lg:block",
      textClass,
    )}
  >
    {file.kind === "directory" ? "-" : formatBytes(file.sizeBytes)}
  </span>
  <span
    class={cn(
      "hidden min-w-0 truncate text-muted-foreground @2xl:block",
      textClass,
    )}
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
      {#if file.sharingMode}
        {#if !bulk}
          <MenuItem onclick={onCopyLink}>
            <LinkIcon size={16} />
            Copy public link
          </MenuItem>
        {/if}
        <span class="block w-full" title={canShare ? undefined : shareDeniedTitle}>
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
        <span class="block w-full" title={canShare ? undefined : shareDeniedTitle}>
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
    </Menu>
    <span class="inline-flex" title={canWrite ? undefined : writeDeniedTitle}>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground hover:text-destructive"
        disabled={!canWrite}
        aria-label={canWrite ? `Delete ${name}` : writeDeniedTitle}
        onclick={onDelete}
      >
        <Trash size={16} />
      </Button>
    </span>
  </div>
</li>

<ContextMenu bind:open={menuOpen} x={menuX} y={menuY} class="w-44">
  <MenuItem onclick={onDownload}>
    <Download size={16} />
    {bulk ? "Download as zip" : "Download"}
  </MenuItem>
  {#if !bulk}
    <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
      <MenuItem
        disabled={!canWrite}
        aria-label={canWrite ? "Rename" : writeDeniedTitle}
        onclick={() => beginRename()}
      >
        <Pencil size={16} />
        Rename
      </MenuItem>
    </span>
  {/if}
  <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
    <MenuItem
      disabled={!canWrite}
      aria-label={canWrite ? "Move" : writeDeniedTitle}
      onclick={onMove}
    >
      <FolderInput size={16} />
      Move
    </MenuItem>
  </span>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="relative"
    onpointerenter={() => {
      clearShareCloseTimer();
      shareOpen = true;
    }}
    onpointerleave={() => {
      scheduleShareClose();
    }}
  >
    <MenuItem closeOnSelect={false}>
      <Share2 size={16} />
      Share
      <ChevronRight size={14} class="ml-auto" />
    </MenuItem>
    {#if shareOpen}
      <div
        role="menu"
        class="absolute top-0 left-full z-50 ml-0.5 w-44 rounded-xl border bg-card p-1 shadow-lg"
      >
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
      </div>
    {/if}
  </div>
  <MenuSeparator />
  <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
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
</ContextMenu>
