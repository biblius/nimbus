<script lang="ts">
  import {
    RotateCcwClock,
    Check,
    ChevronDown,
    Copy,
    Download,
    File as FileIcon,
    Folder,
    FolderInput,
    Link2,
    Plus,
    Search,
    Share2,
    Trash,
    Upload,
    X,
  } from "@lucide/svelte";
  import { PointerActivationConstraints, PointerSensor } from "@dnd-kit/dom";
  import { DragDropProvider, DragOverlay } from "@dnd-kit/svelte";
  import { onDestroy, untrack } from "svelte";
  import { navigate, route } from "../../router";
  import { PreviewQueue } from "../../previewQueue.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import {
    hasShareAccess,
    hasWriteAccess,
    type FileView,
    type Root,
  } from "../../types";
  import { kv, type FilesViewMode } from "../../storage";
  import { UploadHandler } from "../../uploadHandler.svelte";
  import {
    FILE_ICON_CHIP_CLASS,
    fileIconChipStyle,
    isPreviewable,
  } from "../../fileKind";
  import {
    DEFAULT_SORT,
    FileDrafts,
    FileFocus,
    FileListing,
    FileSelection,
    FileSharing,
    FileTransfer,
  } from "../../file-browser";
  import DestinationPicker from "./DestinationPicker.svelte";
  import FileListItem from "./FileListItem.svelte";
  import FilePathCrumbs, { type Crumb } from "./FilePathCrumbs.svelte";
  import FileSearchResults from "./FileSearchResults.svelte";
  import FileShareOptions from "./FileShareOptions.svelte";
  import FileTileItem from "./FileTileItem.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import FileViewModeMenu from "./FileViewModeMenu.svelte";
  import ParentDropItem from "./ParentDropItem.svelte";
  import PendingFileUpload from "./PendingFileUpload.svelte";
  import RootAccessBadges from "./RootAccessBadges.svelte";
  import Button from "../ui/button/button.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import Input from "../ui/input/input.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import Menu from "../ui/menu/menu.svelte";
  import SortHeader from "../ui/sort-header/sort-header.svelte";
  import VirtualScroller from "../ui/virtual-scroller/VirtualScroller.svelte";

  let {
    folder: root,
    parentId = null,
    roots,
  }: {
    folder: Root;
    /** Directory open in this root. Null lists the root itself. */
    parentId?: string | null;
    roots: Root[];
  } = $props();

  const sensors = [
    PointerSensor.configure({
      activationConstraints: [
        new PointerActivationConstraints.Distance({ value: 8 }),
      ],
      preventActivation: (event) =>
        event.target instanceof Element &&
        Boolean(event.target.closest("[data-no-drag]")),
    }),
  ];

  const columns =
    "grid grid-cols-[minmax(0,1fr)_7.5rem] items-center gap-x-3 px-3 @lg:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_7.5rem] @2xl:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_minmax(0,11.5rem)_7.5rem]";

  let searchQuery = $state("");
  const searching = $derived(searchQuery.trim().length > 0);

  /**
   * Controls deletion confirmation dialog.
   */
  let deleteOpen = $state(false);

  /**
   * True when the users has confirmed deletion
   * and the request is processing.
   */
  let deleting = $state(false);

  /**
   * List of file IDs the user has selected.
   * Gets set when they click on delete.
   * Sent to the server for trashing on confirming.
   */
  let pendingDelete = $state<string[]>([]);
  let previewFileId = $state<string | null>(null);
  let viewMode = $state<FilesViewMode>(kv.filesViewMode.get());

  let uploadMenuOpen = $state(false);
  let newMenuOpen = $state(false);

  let fileInput = $state<HTMLInputElement | undefined>();
  let folderInput = $state<HTMLInputElement | undefined>();

  let scroller = $state<HTMLElement | undefined>();
  let viewportWidth = $state(0);

  const canWrite = $derived(hasWriteAccess(root));
  const canShare = $derived(hasShareAccess(root));

  const writableRoots = $derived(roots.filter(hasWriteAccess));

  const hasCopyDestination = $derived(writableRoots.length > 0);

  const writeDeniedTitle = "You do not have write access";
  const shareDeniedTitle = "You do not have share access";

  function copyUnavailableTitle(): string {
    if (hasCopyDestination) {
      return "Copy selected";
    }
    return "There are no writable folders to copy to";
  }

  function directoryShareTitle(): string {
    if (!canShare) {
      return shareDeniedTitle;
    }
    if (listing.locationEntry?.sharingMode) {
      return "Shared";
    }
    return "Share folder";
  }

  function directoryShareAriaLabel(): string {
    if (!canShare) {
      return shareDeniedTitle;
    }
    if (listing.locationEntry?.sharingMode) {
      return "Folder is shared - manage sharing";
    }
    return "Share this folder";
  }

  const tiles = $derived(
    viewMode === "tiles-small" || viewMode === "tiles-large",
  );

  const tileSize = $derived<"small" | "large">(
    viewMode === "tiles-large" ? "large" : "small",
  );

  const listDensity = $derived<"default" | "compact">(
    viewMode === "compact" ? "compact" : "default",
  );

  const virtualLayout = $derived.by(() => {
    if (!tiles) {
      return {
        columns: 1,
        gap: 0,
        itemHeight: listDensity === "compact" ? 44 : 64,
      };
    }

    const large = tileSize === "large";

    const rem =
      typeof document !== "undefined"
        ? parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
        : 16;

    const minTrack = (large ? 9.5 : 6.25) * rem;

    const gap = large ? 8 : 4;

    const pad = large ? 16 : 12;

    const inner = Math.max(0, viewportWidth - pad);

    const columns = Math.max(1, Math.floor((inner + gap) / (minTrack + gap)));

    const colWidth =
      columns > 0 ? (inner - gap * (columns - 1)) / columns : minTrack;

    const itemHeight = colWidth + (large ? 52 : 40);

    return { columns, gap, itemHeight };
  });

  const listing = new FileListing({
    rootId: () => root.id,
    locationId: () => location.id,
    onOpen: () => {
      drafts.reset();
      selection.clearSelection();
      sharing.resetOnOpen();
      previewFileId = null;
      previews.clear();
      scroller?.scrollTo({ top: 0 });
    },
    onLocationChange: (entries) => {
      const key = tiles ? "preview256" : "preview64";

      previews.enqueueAll(
        entries
          .filter(
            (entry) => entry.metadata == null || entry.metadata[key] != null,
          )
          .map((entry) => entry.file),
        false,
      );
    },
  });

  /**
   * Derived from ancestors. Always puts the root in front.
   */
  const trail = $derived.by(() => {
    const crumbs: Crumb[] = [{ id: null, name: root.name }];

    for (const crumb of listing.ancestors) {
      crumbs.push(crumb);
    }

    return crumbs;
  });

  /**
   * Derived from the trail, represents the current location in the file browser,
   * i.e. which directory are we located in.
   */
  const location = $derived(trail[trail.length - 1]);

  const selection = new FileSelection({
    getOrderedIds: () => listing.entries.map((e) => e.file.id),
  });

  const focus = new FileFocus({
    getScroller: () => scroller,
    getVirtualLayout: () => virtualLayout,
    getShowParentDrop: () => showParentDrop,
    onSelect: (ids) => {
      selection.selected = ids;
      selection.selectionAnchor = ids[0] ?? null;
    },
  });

  const drafts = new FileDrafts({
    rootId: () => root.id,
    locationId: () => location.id,
    getCanWrite: () => canWrite,
    getScroller: () => scroller,
    onCreated: (entry) => listing.insertEntry(entry),
  });

  const sharing = new FileSharing({
    rootId: () => root.id,
    getCanShare: () => canShare,
    getLocationEntry: () => listing.locationEntry,
    applyShare: (id, mode, exp) => listing.applyShare(id, mode, exp),
    setSharingModeAll: (mode, exp) => listing.setSharingModeAll(mode, exp),
    onShared: () => selection.clearSelection(),
  });

  const transfer = new FileTransfer({
    root: () => root,
    locationId: () => location.id,
    getCanWrite: () => canWrite,
    getWritableRoots: () => writableRoots,
    getEntries: () => listing.entries,
    getSelected: () => selection.selected,
    label: (e) => listing.label(e),
    removeMovedEntries: (ids) => {
      listing.remove(ids);
      selection.remove(ids);
    },
    exitSelectMode: () => {
      selection.selectMode = false;
    },
    getShowParentDrop: () => showParentDrop,
    getTrailLength: () => trail.length,
    getUpParent: () => upParent,
  });

  const previews = new PreviewQueue({
    rootId: () => root.id,
    locationId: () => location.id,
    getSize: () => (tiles ? 256 : 64),
  });

  const uploads = new UploadHandler({
    rootId: () => root.id,
    locationId: () => location.id,
    onCreated: (entry) => {
      listing.insertEntry(entry);
      previews.enqueue(entry, true);
    },
  });

  /** Parent of the current folder; null when moving into the root. */
  const upParent = $derived(
    trail.length >= 2 ? trail[trail.length - 2].id : null,
  );

  const showParentDrop = $derived(location.id !== null);

  const previewEntries = $derived(listing.entries.filter(isPreviewable));

  const deleteDescription = $derived.by(() => {
    if (pendingDelete.length === 1) {
      const entry = listing.entries.find(
        (item) => item.file.id === pendingDelete[0],
      );
      return `Delete ${entry ? listing.label(entry) : "this item"}?`;
    }
    return `Delete ${pendingDelete.length} items?`;
  });

  $effect(() => {
    // Deliberate assignment so $effect keeps track of these
    const rootId = root.id;
    const parent = parentId;

    // Untrack the open function, since it mutates state and would trigger
    // the effect indefinitely.
    untrack(() => {
      listing.open(rootId, parent);
    });
  });

  $effect(() => {
    root.id;
    searchQuery = "";
  });

  $effect(() => {
    const next = tiles;

    if (seenTiles !== undefined && seenTiles !== next) {
      previews.refreshAll(listing.entries.map((entry) => entry.file));
    }

    seenTiles = next;
  });

  $effect(() => {
    const node = scroller;

    if (!node) {
      viewportWidth = 0;
      return;
    }

    const ro = new ResizeObserver(() => {
      viewportWidth = node.clientWidth;
    });

    ro.observe(node);
    viewportWidth = node.clientWidth;

    return () => ro.disconnect();
  });

  /**
   * `?file=ID` on a directory route: select + scroll to the row, or redirect
   * to the file's parent directory when the current folder is wrong.
   */
  $effect(() => {
    const raw = route.search.file;
    const fileId =
      raw === undefined || raw === null || raw === "" ? null : String(raw);
    return focus.apply({
      fileId,
      rootId: root.id,
      parentId,
      list: listing.entries,
      isLoading: listing.loading,
    });
  });

  function showDirectory(
    parent: string | null,
    search?: Record<string, string>,
  ) {
    if (parent) {
      navigate("/files/:root_id/dir/:parent_id", {
        params: { root_id: root.id, parent_id: parent },
        search,
      });
      return;
    }
    navigate("/files/:root_id", { params: { root_id: root.id }, search });
  }

  function openDirectory(entry: FileView) {
    listing.openDirectory(entry, parentId, showDirectory);
  }

  function openParentDirectory() {
    listing.openParentDirectory(trail.length, upParent, showDirectory);
  }

  // Swap cached thumbs when switching between list (64) and tiles (256).
  let seenTiles: boolean | undefined;

  onDestroy(() => previews.clear());

  function onWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") {
      return;
    }

    if (
      event.defaultPrevented ||
      uploadMenuOpen ||
      newMenuOpen ||
      deleteOpen ||
      sharing.shareOpen ||
      previewFileId !== null ||
      drafts.drafting
    ) {
      return;
    }

    if (!selection.selectMode && selection.selected.length === 0) {
      return;
    }

    event.preventDefault();

    selection.clearSelection();
  }

  function startDelete(ids: string[]) {
    if (!canWrite || ids.length === 0) {
      return;
    }

    pendingDelete = ids.slice();
    deleteOpen = true;
  }

  function onWindowFileDragEnter(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragEnter(event);
  }

  function onWindowFileDragLeave(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragLeave(event);
  }

  function onWindowFileDragOver(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDragOver(event);
  }

  function onWindowFileDrop(event: DragEvent) {
    if (!canWrite) {
      return;
    }
    uploads.onWindowDrop(event);
  }

  async function confirmDelete() {
    const ids = pendingDelete;
    const rootId = root.id;
    const parent = location.id;

    deleting = true;

    try {
      await session.client.trash(rootId, ids);

      if (root.id !== rootId || location.id !== parent) {
        return;
      }

      listing.remove(ids);
      pendingDelete = [];
      deleteOpen = false;
      selection.clearSelection();
    } catch (cause) {
      const single =
        ids.length === 1
          ? listing.entries.find((item) => item.file.id === ids[0])
          : undefined;

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : single
            ? `Could not delete ${listing.label(single)}`
            : `Could not delete ${ids.length} items`,
      );
    } finally {
      if (root.id === rootId && location.id === parent) {
        deleting = false;
      }
    }
  }

  function downloadName(entry: FileView) {
    return entry.file.kind === "directory"
      ? `${entry.file.name}.zip`
      : entry.file.name;
  }

  async function download(entry: FileView) {
    try {
      await session.client.download(
        root.id,
        entry.file.id,
        downloadName(entry),
      );
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${listing.label(entry)}`,
      );
    }
  }

  /** One id uses the normal download; several become a single zip. */
  async function downloadIds(ids: string[]) {
    if (ids.length === 0) {
      return;
    }

    if (ids.length === 1) {
      const entry = listing.entries.find((item) => item.file.id === ids[0]);

      if (entry) {
        await download(entry);
        selection.clearSelection();
      }

      return;
    }

    try {
      await session.client.downloadDir(root.id, ids, "download.zip");
      selection.clearSelection();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${ids.length} items`,
      );
    }
  }

  async function downloadCurrent() {
    try {
      if (listing.locationEntry) {
        await session.client.download(
          root.id,
          listing.locationEntry.id,
          `${listing.locationEntry.name}.zip`,
        );
      } else {
        await session.client.downloadRoot(root.id, `${root.name}.zip`);
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${listing.locationEntry?.name ?? root.name}`,
      );
    }
  }
</script>

<svelte:window
  onkeydown={onWindowKeydown}
  ondragenter={onWindowFileDragEnter}
  ondragleave={onWindowFileDragLeave}
  ondragover={onWindowFileDragOver}
  ondrop={onWindowFileDrop}
/>

<div class="relative flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  {#if uploads.fileDragActive}
    <div
      class="pointer-events-none absolute inset-0 z-50 flex items-center justify-center rounded-lg border-2 border-dashed border-border bg-card/90"
      aria-hidden="true"
    >
      <p class="text-sm font-medium text-muted-foreground">
        Drop files or folders to upload
      </p>
    </div>
  {/if}
  <div class="flex shrink-0 flex-col gap-2.5">
    <!-- PATH: always full-width above the list -->
    <div class="flex min-w-0 items-center gap-2 px-0.5">
      {#if root.owner == null}
        <RootAccessBadges accessMode={root.accessMode} />
      {/if}
      <FilePathCrumbs
        crumbs={trail}
        onNavigate={(index, crumb) => {
          listing.sliceAncestors(index);
          showDirectory(crumb.id);
        }}
      />
    </div>

    <!-- TOOLS: search + actions; stacked on small screens so menus are not clipped -->
    <div class="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-stretch">
      <div
        class="relative min-w-0 flex-1 overflow-hidden rounded-xl border border-border bg-card shadow-sm"
      >
        <Search
          size={15}
          class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          type="search"
          class="h-9 border-0 bg-transparent pl-9 shadow-none focus-visible:ring-0"
          placeholder="Search files…"
          bind:value={searchQuery}
          aria-label="Search files in this root"
        />
      </div>

      <div
        class="relative z-10 flex w-full shrink-0 items-center justify-evenly gap-0.5 rounded-xl border border-border bg-card p-0.5 shadow-sm sm:w-auto sm:justify-start sm:self-start"
      >
        <input
          bind:this={fileInput}
          type="file"
          class="sr-only"
          multiple
          tabindex={-1}
          aria-hidden="true"
          onchange={(event) => uploads.fromFileInput(event)}
        />
        <input
          bind:this={folderInput}
          type="file"
          class="sr-only"
          multiple
          webkitdirectory
          tabindex={-1}
          aria-hidden="true"
          onchange={(event) => uploads.fromFolderInput(event)}
        />
        {#if selection.selectMode}
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="Clear selection"
            title="Clear selection"
            onclick={() => selection.clearSelection()}
          >
            <X size={15} />
          </Button>
          {#if selection.selected.length > 0}
            <span
              class="inline-flex"
              title={hasCopyDestination
                ? `Copy ${selection.selected.length} selected`
                : copyUnavailableTitle()}
            >
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                disabled={!hasCopyDestination}
                aria-label={hasCopyDestination
                  ? `Copy ${selection.selected.length} selected`
                  : copyUnavailableTitle()}
                title={hasCopyDestination
                  ? `Copy ${selection.selected.length} selected`
                  : copyUnavailableTitle()}
                onclick={() => transfer.startCopy(selection.selected)}
              >
                <Copy size={15} />
              </Button>
            </span>
            <span
              class="inline-flex"
              title={canWrite
                ? `Move ${selection.selected.length} selected`
                : writeDeniedTitle}
            >
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                disabled={!canWrite}
                aria-label={canWrite
                  ? `Move ${selection.selected.length} selected`
                  : writeDeniedTitle}
                title={canWrite
                  ? `Move ${selection.selected.length} selected`
                  : writeDeniedTitle}
                onclick={() => transfer.startMove(selection.selected)}
              >
                <FolderInput size={15} />
              </Button>
            </span>
            <span
              class="inline-flex"
              title={canWrite
                ? `Delete ${selection.selected.length} selected`
                : writeDeniedTitle}
            >
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                disabled={!canWrite}
                aria-label={canWrite
                  ? `Delete ${selection.selected.length} selected`
                  : writeDeniedTitle}
                title={canWrite
                  ? `Delete ${selection.selected.length} selected`
                  : writeDeniedTitle}
                onclick={() => startDelete(selection.selected)}
              >
                <Trash size={15} />
              </Button>
            </span>
          {/if}
        {:else}
          <Menu bind:open={uploadMenuOpen}>
            {#snippet trigger(props, open)}
              <span
                class="inline-flex"
                title={canWrite ? "Upload" : writeDeniedTitle}
              >
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  class="w-auto gap-0.5 px-1.5"
                  disabled={!canWrite}
                  aria-label={canWrite ? "Upload" : writeDeniedTitle}
                  title={canWrite ? "Upload" : writeDeniedTitle}
                  {...props}
                  onclick={() => {
                    if (canWrite) {
                      props.onclick();
                    }
                  }}
                >
                  <Upload size={15} />
                  <ChevronDown
                    size={14}
                    class={[
                      "text-muted-foreground transition-transform",
                      open && "rotate-180",
                    ]}
                  />
                </Button>
              </span>
            {/snippet}
            <MenuItem onclick={() => fileInput?.click()}>
              <FileIcon size={15} />
              File
            </MenuItem>
            <MenuItem onclick={() => folderInput?.click()}>
              <Folder size={15} />
              Folder
            </MenuItem>
          </Menu>
          <Menu bind:open={newMenuOpen}>
            {#snippet trigger(props, open)}
              <span
                class="inline-flex"
                title={canWrite ? "New" : writeDeniedTitle}
              >
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  class="w-auto gap-0.5 px-1.5"
                  disabled={!canWrite}
                  aria-label={canWrite ? "New" : writeDeniedTitle}
                  title={canWrite ? "New" : writeDeniedTitle}
                  {...props}
                  onclick={() => {
                    if (canWrite) {
                      props.onclick();
                    }
                  }}
                >
                  <Plus size={15} />
                  <ChevronDown
                    size={14}
                    class={[
                      "text-muted-foreground transition-transform",
                      open && "rotate-180",
                    ]}
                  />
                </Button>
              </span>
            {/snippet}
            <MenuItem onclick={() => drafts.beginDraft("text")}>
              <FileIcon size={15} />
              Text file
            </MenuItem>
            <MenuItem onclick={() => drafts.beginDraft("directory")}>
              <Folder size={15} />
              Directory
            </MenuItem>
          </Menu>
        {/if}
        <div
          class="mx-0.5 w-px self-stretch bg-border"
          aria-hidden="true"
        ></div>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="text-muted-foreground"
          aria-label={listing.locationEntry
            ? `Download ${listing.locationEntry.name}`
            : `Download ${root.name}`}
          title={listing.locationEntry ? "Download folder" : "Download root"}
          onclick={downloadCurrent}
        >
          <Download size={15} />
        </Button>
        <div
          class="mx-0.5 w-px self-stretch bg-border"
          aria-hidden="true"
        ></div>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="text-muted-foreground"
          aria-label="Activity"
          title="Activity"
          onclick={() =>
            navigate("/files/:root_id/activity", {
              params: { root_id: root.id },
            })}
        >
          <RotateCcwClock size={15} />
        </Button>
        {#if listing.locationEntry}
          <div
            class="mx-0.5 w-px self-stretch bg-border"
            aria-hidden="true"
          ></div>
          <span class="inline-flex" title={directoryShareTitle()}>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class={listing.locationEntry.sharingMode
                ? "text-primary hover:text-primary"
                : "text-muted-foreground"}
              disabled={!canShare}
              aria-label={directoryShareAriaLabel()}
              title={directoryShareTitle()}
              onclick={() => sharing.openDirectoryShareDialog()}
            >
              {#if listing.locationEntry.sharingMode}
                <Link2 size={15} />
              {:else}
                <Share2 size={15} />
              {/if}
            </Button>
          </span>
        {/if}
      </div>
    </div>
  </div>

  {#if uploads.pending.length > 0}
    <ul
      class="max-h-40 shrink-0 space-y-2 overflow-y-auto rounded-lg border bg-card p-3"
    >
      {#each uploads.pending as item (item.id)}
        <PendingFileUpload
          {item}
          percent={uploads.percent(item)}
          onCancel={() => uploads.cancel(item.id)}
          onDismiss={() => uploads.dismiss(item.id)}
        />
      {/each}
    </ul>
  {/if}

  <div
    class="@container flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card"
  >
    {#if searching}
      <div
        class="flex h-10 shrink-0 items-center border-b px-3 text-xs font-medium text-muted-foreground"
      >
        Search results
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        <FileSearchResults
          {root}
          query={searchQuery}
          onNavigate={(parentId, fileId) => {
            searchQuery = "";
            // File picks set `?file=`; directory picks clear it so no stale highlight.
            showDirectory(parentId, fileId ? { file: fileId } : {});
          }}
        />
      </div>
    {:else}
      <!-- COLUMN HEADERS -->

      {#if tiles}
        <div class="flex h-10 shrink-0 items-center justify-end border-b px-3">
          <FileViewModeMenu bind:mode={viewMode} />
        </div>
      {:else}
        <div
          class="{columns} h-10 shrink-0 border-b text-xs font-medium text-muted-foreground"
        >
          <SortHeader
            label="Name"
            column="name"
            bind:sort={listing.sort}
            defaultSort={DEFAULT_SORT}
            onCycle={() => listing.onSortCycle()}
          />
          <SortHeader
            label="Size"
            column="size"
            bind:sort={listing.sort}
            defaultSort={DEFAULT_SORT}
            onCycle={() => listing.onSortCycle()}
            class="hidden @lg:flex"
            buttonClass="justify-end"
          />
          <SortHeader
            label="Created"
            column="created"
            bind:sort={listing.sort}
            defaultSort={DEFAULT_SORT}
            onCycle={() => listing.onSortCycle()}
            class="hidden @2xl:flex"
          />
          <div class="flex items-center justify-end">
            <FileViewModeMenu bind:mode={viewMode} />
          </div>
        </div>
      {/if}

      <!-- FILE LIST -->

      {#if listing.loading}
        <div
          bind:this={scroller}
          class="min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <p
            class="flex h-full items-center justify-center px-3 text-sm text-muted-foreground"
          >
            Loading…
          </p>
        </div>
      {:else if listing.entries.length === 0 && !drafts.drafting && !showParentDrop}
        <div
          bind:this={scroller}
          class="min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <div
            class="flex h-full flex-col items-center justify-center gap-2 px-6 py-16 text-center"
          >
            <Folder class="size-10 text-muted-foreground opacity-40" />
            <p class="text-sm font-medium">This folder is empty</p>
            <p class="max-w-sm text-sm text-muted-foreground">
              Upload or drop files here, or create a folder to get started.
            </p>
          </div>
        </div>
      {:else}
        {#snippet parentLeading()}
          <ParentDropItem {columns} {viewMode} onOpen={openParentDirectory} />
        {/snippet}
        <DragDropProvider {sensors} onDragEnd={(e) => transfer.onDragEnd(e)}>
          <VirtualScroller
            bind:element={scroller}
            items={listing.entries}
            itemHeight={virtualLayout.itemHeight}
            columns={virtualLayout.columns}
            gap={virtualLayout.gap}
            getKey={(entry) => entry.file.id}
            as="ul"
            class={tiles
              ? tileSize === "large"
                ? "p-2"
                : "p-1.5 sm:p-2"
              : undefined}
            windowClass={tiles ? undefined : "divide-y"}
            leading={showParentDrop ? parentLeading : undefined}
          >
            {#snippet header()}
              {#if drafts.drafting}
                <ul class={tiles ? undefined : "divide-y"}>
                  <li class={["px-3 py-2", tiles && "col-span-full"]}>
                    <div class="flex min-w-0 items-center gap-2">
                      {#if drafts.draftKind === "text"}
                        <FileIcon
                          size={18}
                          class="shrink-0 text-muted-foreground"
                        />
                      {:else}
                        <Folder size={18} class="shrink-0 text-primary" />
                      {/if}
                      <Input
                        {@attach drafts.focusDraft}
                        bind:value={drafts.directoryName}
                        class="h-8 min-w-0 flex-1 px-2.5"
                        aria-label={drafts.draftKind === "text"
                          ? "File name"
                          : "Folder name"}
                        aria-invalid={drafts.directoryError ? true : undefined}
                        autocomplete="off"
                        spellcheck="false"
                        placeholder={drafts.draftKind === "text"
                          ? "File name"
                          : "Folder name"}
                        disabled={drafts.directorySaving}
                        oninput={() => (drafts.directoryError = "")}
                        onkeydown={(e) => drafts.onDraftKey(e)}
                      />
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label={drafts.draftKind === "text"
                          ? "Create file"
                          : "Create folder"}
                        disabled={drafts.directorySaving}
                        onclick={() => drafts.commitDirectory()}
                      >
                        <Check size={16} />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Cancel"
                        disabled={drafts.directorySaving}
                        onclick={() => drafts.cancelDirectory()}
                      >
                        <X size={16} />
                      </Button>
                    </div>
                    <p
                      class="mt-1 pl-7 text-xs {drafts.directoryError
                        ? 'text-destructive'
                        : 'text-muted-foreground'}"
                    >
                      {drafts.directoryError ||
                        (drafts.directorySaving
                          ? "Saving…"
                          : "Enter to save, Esc to cancel")}
                    </p>
                  </li>
                </ul>
              {/if}
            {/snippet}
            {#snippet children(entry)}
              {#if tiles}
                <FileTileItem
                  {entry}
                  thumbnailUrl={previews.thumbs[entry.file.id]}
                  name={listing.label(entry)}
                  size={tileSize}
                  selected={selection.selected.includes(entry.file.id)}
                  multiSelected={selection.selected.length >= 2}
                  selectMode={selection.selectMode}
                  {canWrite}
                  {canShare}
                  canCopy={hasCopyDestination}
                  onSelect={(event) =>
                    selection.selectEntry(entry.file.id, event)}
                  onLongSelect={() => selection.longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onDownload={() =>
                    downloadIds(transfer.actionIds(entry.file.id))}
                  onCopy={() =>
                    transfer.startCopy(transfer.actionIds(entry.file.id))}
                  onShare={() =>
                    sharing.openShareDialog(
                      entry.file,
                      transfer.actionIds(entry.file.id),
                    )}
                  onUnshare={() =>
                    sharing.openShareDialog(
                      entry.file,
                      transfer.actionIds(entry.file.id),
                    )}
                  onCopyLink={() => sharing.copyLink(entry)}
                  onMove={() =>
                    transfer.startMove(transfer.actionIds(entry.file.id))}
                  onDelete={() =>
                    startDelete(transfer.actionIds(entry.file.id))}
                />
              {:else}
                <FileListItem
                  {entry}
                  thumbnailUrl={previews.thumbs[entry.file.id]}
                  name={listing.label(entry)}
                  {columns}
                  density={listDensity}
                  selected={selection.selected.includes(entry.file.id)}
                  multiSelected={selection.selected.length >= 2}
                  selectMode={selection.selectMode}
                  {canWrite}
                  {canShare}
                  canCopy={hasCopyDestination}
                  onSelect={(event) =>
                    selection.selectEntry(entry.file.id, event)}
                  onLongSelect={() => selection.longSelectEntry(entry.file.id)}
                  onOpen={() => openDirectory(entry)}
                  onPreview={() => (previewFileId = entry.file.id)}
                  onDownload={() =>
                    downloadIds(transfer.actionIds(entry.file.id))}
                  onCopy={() =>
                    transfer.startCopy(transfer.actionIds(entry.file.id))}
                  onShare={() =>
                    sharing.openShareDialog(
                      entry.file,
                      transfer.actionIds(entry.file.id),
                    )}
                  onUnshare={() =>
                    sharing.openShareDialog(
                      entry.file,
                      transfer.actionIds(entry.file.id),
                    )}
                  onCopyLink={() => sharing.copyLink(entry)}
                  onMove={() =>
                    transfer.startMove(transfer.actionIds(entry.file.id))}
                  onRename={(name) =>
                    listing.renameEntry(entry, name, canWrite)}
                  onDelete={() =>
                    startDelete(transfer.actionIds(entry.file.id))}
                />
              {/if}
            {/snippet}
          </VirtualScroller>
          <DragOverlay>
            {#snippet children(source)}
              {@const preview = transfer.dragPreview(String(source.id))}
              <div class="relative max-w-72 -rotate-1">
                {#if preview.count > 1}
                  <div
                    class="absolute inset-0 translate-x-1.5 translate-y-1.5 rounded-lg border bg-muted/80 ring-1 ring-border"
                    aria-hidden="true"
                  ></div>
                {/if}
                <div
                  class="relative flex items-center gap-2.5 rounded-lg border bg-card py-1.5 pr-2.5 pl-1.5 shadow-xl ring-1 ring-border"
                >
                  {#if preview.entry?.file.kind === "directory"}
                    <span
                      class="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
                    >
                      <Folder size={18} strokeWidth={1.75} />
                    </span>
                  {:else}
                    <span
                      class="flex size-8 shrink-0 items-center justify-center rounded-md ring-1 {FILE_ICON_CHIP_CLASS}"
                      style={fileIconChipStyle(preview.entry?.file.fileType)}
                    >
                      <FileTypeIcon
                        fileType={preview.entry?.file.fileType}
                        size={16}
                      />
                    </span>
                  {/if}
                  <span
                    class="min-w-0 flex-1 truncate text-sm font-medium"
                    title={preview.name}
                  >
                    {preview.name}
                  </span>
                  {#if preview.count > 1}
                    <span
                      class="shrink-0 rounded-full bg-primary px-2 py-0.5 text-xs font-semibold text-primary-foreground tabular-nums"
                    >
                      +{preview.count - 1}
                    </span>
                  {/if}
                </div>
              </div>
            {/snippet}
          </DragOverlay>
        </DragDropProvider>
      {/if}
    {/if}
  </div>

  {#if previewFileId !== null && previewEntries.some((entry) => entry.file.id === previewFileId)}
    <!-- 
    Use dynamic import here so we don't load preview libs (pdfjs, codemirror, sheetjs)
    on the initial file browser load
    -->
    {#await import("./FilePreview.svelte") then { default: FilePreview }}
      <FilePreview
        rootId={root.id}
        files={previewEntries}
        fileId={previewFileId}
        readonly={!canWrite}
        onClose={() => (previewFileId = null)}
        onSelect={(id) => (previewFileId = id)}
        onUpdated={(sizeBytes) => {
          if (previewFileId !== null) {
            listing.setEntrySize(previewFileId, sizeBytes);
          }
        }}
      />
    {/await}
  {/if}

  <ConfirmDialog
    open={deleteOpen}
    title={pendingDelete.length === 1 ? "Delete item" : "Delete items"}
    description={deleteDescription}
    busy={deleting}
    onCancel={() => {
      if (!deleting) {
        deleteOpen = false;
      }
    }}
    onConfirm={confirmDelete}
  />

  {#if sharing.shareOpen && sharing.shareSubject}
    <FileShareOptions
      entry={sharing.shareSubject}
      busy={sharing.shareBusy}
      onCancel={() => sharing.closeShareDialog()}
      onShare={(input) => sharing.confirmShare(input)}
      onUnshare={() => sharing.confirmUnshare()}
    />
  {/if}
</div>

<DestinationPicker
  request={transfer.destination}
  onCancel={() => (transfer.destination = null)}
/>
