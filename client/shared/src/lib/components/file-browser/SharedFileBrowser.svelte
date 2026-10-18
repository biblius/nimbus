<script lang="ts">
  import { Download, Eye, Folder } from "@lucide/svelte";
  import { onDestroy, untrack } from "svelte";
  import { navigate } from "../../router";
  import { PreviewQueue } from "../../previewQueue.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import { kv, type FilesViewMode } from "../../storage";
  import type { SharedFileView } from "../../types";
  import { hasListThumbnail, isPreviewable } from "../../fileKind";
  import { formatBytes, formatDatetime } from "../../format";
  import { cn } from "../../utils";
  import FilePathCrumbs, { type Crumb } from "./FilePathCrumbs.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";
  import FileViewModeMenu from "./FileViewModeMenu.svelte";
  import Button from "../ui/button/button.svelte";

  /** Guards against cycles / absurd depth when walking ancestors. */
  const MAX_DEPTH = 64;

  let {
    shareRoot,
    parentId = null,
  }: {
    /** The publicly shared directory this page was opened with. */
    shareRoot: SharedFileView;
    /** Directory open inside the share. Null lists the share root itself. */
    parentId?: string | null;
  } = $props();

  const columns =
    "grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 px-3 @lg:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_auto] @2xl:grid-cols-[minmax(8rem,1fr)_minmax(0,5.5rem)_minmax(0,11.5rem)_auto]";

  const canDownload = $derived(shareRoot.file.sharingMode === "download");

  let entries = $state<SharedFileView[]>([]);
  let loading = $state(true);
  let latestLoad = 0;

  let ancestors = $state<Crumb[]>([]);
  let previewFileId = $state<string | null>(null);
  let viewMode = $state<FilesViewMode>(kv.filesViewMode.get());

  const shareRootId = $derived(shareRoot.file.id);
  const locationId = $derived(parentId ?? shareRootId);
  const previewEntries = $derived(entries.filter(isPreviewable));

  const tiles = $derived(
    viewMode === "tiles-small" || viewMode === "tiles-large",
  );
  const compact = $derived(viewMode === "compact");
  const large = $derived(viewMode === "tiles-large");

  const tileGridClass = $derived(
    large
      ? "grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-1 p-2 sm:gap-2"
      : "grid grid-cols-[repeat(auto-fill,minmax(6.25rem,1fr))] gap-0.5 p-1.5 sm:gap-1 sm:p-2",
  );

  const trail = $derived<Crumb[]>([
    { id: shareRootId, name: shareRoot.file.name },
    ...ancestors,
  ]);

  const previews = new PreviewQueue({
    getRootId: () => shareRootId,
    getParentId: () => locationId,
    getSize: () => (tiles ? 256 : 64),
    fetchPreview: (id, size) => session.client.sharedPreview(id, size),
  });

  // Swap cached thumbs when switching between list (64) and tiles (256).
  let seenTiles: boolean | undefined;

  $effect(() => {
    const next = tiles;

    if (seenTiles !== undefined && seenTiles !== next) {
      untrack(() => queueThumbnails());
    }

    seenTiles = next;
  });

  $effect(() => {
    const id = shareRootId;
    const parent = parentId;

    untrack(() => {
      open(id, parent);
    });
  });

  onDestroy(() => previews.clear());

  function compare(left: SharedFileView, right: SharedFileView) {
    const byKind = left.file.kind.localeCompare(right.file.kind);

    if (byKind !== 0) {
      return byKind;
    }

    return left.file.name.localeCompare(right.file.name, undefined, {
      numeric: true,
      sensitivity: "base",
    });
  }

  function queueThumbnails() {
    const key = tiles ? "preview256" : "preview64";

    previews.refreshAll(
      entries
        .filter((entry) => entry.metadata == null || entry.metadata[key] != null)
        .map((entry) => entry.file),
    );
  }

  function showDirectory(id: string) {
    if (id === shareRootId) {
      navigate("/files/shared/:file_id", { params: { file_id: shareRootId } });
      return;
    }

    navigate("/files/shared/:file_id/dir/:parent_id", {
      params: { file_id: shareRootId, parent_id: id },
    });
  }

  /**
   * Walk from `start` up to the share root through `parent` links.
   * Returns crumbs below the share root (inclusive of `start`).
   */
  async function loadAncestors(start: string, currentLoad: number) {
    const chain: Crumb[] = [];
    let cursor: string | null = start;

    for (let depth = 0; depth < MAX_DEPTH; depth++) {
      if (cursor === null || cursor === shareRootId) {
        break;
      }

      const view = await session.client.sharedFile(cursor);

      if (currentLoad !== latestLoad) {
        return;
      }

      chain.unshift({ id: view.file.id, name: view.file.name });
      cursor = view.file.parent;
    }

    ancestors = chain;
  }

  async function open(id: string, parent: string | null) {
    const currentLoad = ++latestLoad;

    loading = true;
    entries = [];
    previewFileId = null;
    previews.clear();

    const target = parent ?? id;

    if (parent === null) {
      ancestors = [];
    } else if (ancestors.at(-1)?.id !== parent) {
      loadAncestors(parent, currentLoad).catch(() => {
        if (currentLoad !== latestLoad) {
          return;
        }

        ancestors = [];
        toastError("Could not open this folder");
      });
    }

    try {
      const next = await session.client.sharedList(target);

      if (currentLoad !== latestLoad) {
        return;
      }

      entries = next.sort(compare);

      queueThumbnails();
    } catch (cause) {
      if (currentLoad !== latestLoad) {
        return;
      }

      entries = [];

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not list this folder",
      );
    } finally {
      if (currentLoad === latestLoad) {
        loading = false;
      }
    }
  }

  function activate(entry: SharedFileView) {
    if (entry.file.kind === "directory") {
      const id = entry.file.id;

      if (ancestors.at(-1)?.id !== id) {
        ancestors = ancestors.concat({ id, name: entry.file.name });
      }

      showDirectory(id);
      return;
    }

    if (isPreviewable(entry)) {
      previewFileId = entry.file.id;
    }
  }

  function downloadName(entry: SharedFileView) {
    return entry.file.kind === "directory"
      ? `${entry.file.name}.zip`
      : entry.file.name;
  }

  async function download(entry: SharedFileView) {
    if (!canDownload) {
      return;
    }

    try {
      await session.client.sharedDownload(entry.file.id, downloadName(entry));
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${entry.file.name}`,
      );
    }
  }

  async function downloadCurrent() {
    if (!canDownload) {
      return;
    }

    const id = locationId;
    const folderName = trail.at(-1)?.name ?? shareRoot.file.name;
    const name = `${folderName}.zip`;

    try {
      await session.client.sharedDownload(id, name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${folderName}`,
      );
    }
  }
</script>

{#snippet thumb(entry: SharedFileView, boxClass: string, iconSize: number)}
  {@const thumbnailUrl = previews.thumbs[entry.file.id]}
  {#if entry.file.kind === "directory"}
    <span
      class={cn(
        "flex items-center justify-center rounded-lg bg-primary/10 text-primary",
        boxClass,
      )}
    >
      <Folder size={iconSize} strokeWidth={1.75} />
    </span>
  {:else}
    <span
      class={cn(
        "group relative flex items-center justify-center overflow-hidden rounded-lg bg-muted text-muted-foreground ring-1 ring-border/60",
        boxClass,
      )}
    >
      {#if hasListThumbnail(entry) && thumbnailUrl}
        <img src={thumbnailUrl} alt="" class="size-full object-cover" />
      {:else}
        <FileTypeIcon fileType={entry.file.fileType} size={iconSize} />
      {/if}
      {#if isPreviewable(entry)}
        <span
          class="pointer-events-none absolute inset-0 flex items-center justify-center bg-background/80 text-foreground opacity-0 transition-opacity group-hover:opacity-100"
          aria-hidden="true"
        >
          <Eye size={Math.min(iconSize, 20)} strokeWidth={2} />
        </span>
      {/if}
    </span>
  {/if}
{/snippet}

<div class="relative flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  <div class="flex shrink-0 flex-wrap items-center justify-between gap-2">
    <FilePathCrumbs
      crumbs={trail}
      onNavigate={(index, crumb) => {
        if (crumb.id === null) {
          return;
        }

        ancestors = ancestors.slice(0, index);
        showDirectory(crumb.id);
      }}
    />
    {#if canDownload}
      <Button
        type="button"
        variant="outline"
        size="sm"
        class="text-muted-foreground"
        aria-label="Download folder"
        title="Download folder"
        onclick={downloadCurrent}
      >
        <Download size={15} />
        <span class="hidden sm:inline">Download</span>
      </Button>
    {/if}
  </div>

  <div
    class="@container flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card"
  >
    {#if tiles}
      <div class="flex h-10 shrink-0 items-center justify-end border-b px-3">
        <FileViewModeMenu bind:mode={viewMode} />
      </div>
    {:else}
      <div
        class="{columns} h-10 shrink-0 border-b text-xs font-medium text-muted-foreground"
      >
        <span class="truncate">Name</span>
        <span class="hidden text-right @lg:block">Size</span>
        <span class="hidden @2xl:block">Created</span>
        <div class="flex items-center justify-end">
          <FileViewModeMenu bind:mode={viewMode} />
        </div>
      </div>
    {/if}

    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
      {#if loading}
        <p
          class="flex h-full items-center justify-center px-3 text-sm text-muted-foreground"
        >
          Loading…
        </p>
      {:else if entries.length === 0}
        <div
          class="flex h-full flex-col items-center justify-center gap-2 px-6 py-16 text-center"
        >
          <Folder class="size-10 text-muted-foreground opacity-40" />
          <p class="text-sm font-medium">This folder is empty</p>
        </div>
      {:else}
        <ul class={tiles ? tileGridClass : "divide-y"}>
          {#each entries as entry (entry.file.id)}
            {@const openable =
              entry.file.kind === "directory" || isPreviewable(entry)}
            {#if tiles}
              <li
                class={cn(
                  "flex flex-col rounded-lg hover:bg-accent/60",
                  large ? "gap-1.5 p-2" : "gap-1 p-1.5",
                )}
              >
                <button
                  type="button"
                  class="flex min-w-0 flex-col gap-1 rounded-md text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none enabled:cursor-pointer"
                  disabled={!openable}
                  aria-label={entry.file.kind === "directory"
                    ? `Open ${entry.file.name}`
                    : `Preview ${entry.file.name}`}
                  onclick={() => activate(entry)}
                >
                  {@render thumb(
                    entry,
                    "aspect-square w-full",
                    large ? 44 : 30,
                  )}
                  <span
                    class={cn(
                      "w-full text-center font-medium",
                      large ? "line-clamp-2 text-sm" : "truncate text-xs",
                    )}
                    title={entry.file.name}
                  >
                    {entry.file.name}
                  </span>
                </button>
                <div class="flex items-center justify-center gap-1">
                  <span class="text-[11px] text-muted-foreground tabular-nums">
                    {entry.file.kind === "directory"
                      ? "-"
                      : formatBytes(entry.file.sizeBytes)}
                  </span>
                  {#if canDownload}
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      class="size-6 text-muted-foreground"
                      aria-label="Download {entry.file.name}"
                      onclick={() => download(entry)}
                    >
                      <Download size={14} />
                    </Button>
                  {/if}
                </div>
              </li>
            {:else}
              <li
                class={cn(
                  columns,
                  compact ? "min-h-11 py-1" : "min-h-16 py-2",
                  "hover:bg-accent/60",
                )}
              >
                <button
                  type="button"
                  class={cn(
                    "flex w-full min-w-0 items-center rounded-md text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none enabled:cursor-pointer",
                    compact ? "min-h-11 gap-2.5" : "min-h-14 gap-3",
                  )}
                  disabled={!openable}
                  aria-label={entry.file.kind === "directory"
                    ? `Open ${entry.file.name}`
                    : `Preview ${entry.file.name}`}
                  onclick={() => activate(entry)}
                >
                  {@render thumb(
                    entry,
                    cn("shrink-0", compact ? "size-11" : "size-14"),
                    compact ? 22 : 30,
                  )}
                  <span
                    class={cn(
                      "min-w-0 flex-1 truncate font-medium",
                      compact ? "text-xs" : "text-sm",
                    )}
                    title={entry.file.name}
                  >
                    {entry.file.name}
                  </span>
                </button>
                <span
                  class={cn(
                    "hidden min-w-0 overflow-hidden text-right text-muted-foreground tabular-nums @lg:block",
                    compact ? "text-xs" : "text-sm",
                  )}
                >
                  {entry.file.kind === "directory"
                    ? "-"
                    : formatBytes(entry.file.sizeBytes)}
                </span>
                <span
                  class={cn(
                    "hidden min-w-0 truncate text-muted-foreground @2xl:block",
                    compact ? "text-xs" : "text-sm",
                  )}
                >
                  {formatDatetime(entry.file.createdAt)}
                </span>
                <div class="flex min-w-9 items-center justify-end gap-1">
                  {#if canDownload}
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      class="text-muted-foreground"
                      aria-label="Download {entry.file.name}"
                      onclick={() => download(entry)}
                    >
                      <Download size={16} />
                    </Button>
                  {/if}
                </div>
              </li>
            {/if}
          {/each}
        </ul>
      {/if}
    </div>
  </div>

  {#if previewFileId !== null && previewEntries.some((entry) => entry.file.id === previewFileId)}
    <!-- Dynamic import so pdfjs / codemirror / sheetjs load only when previewing. -->
    {#await import("./SharedFilePreview.svelte") then { default: SharedFilePreview }}
      <SharedFilePreview
        files={previewEntries}
        fileId={previewFileId}
        onClose={() => (previewFileId = null)}
        onSelect={(id) => (previewFileId = id)}
      />
    {/await}
  {/if}
</div>
