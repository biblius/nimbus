<script lang="ts">
  import { Download, Eye, Folder, Share2, Trash } from "@lucide/svelte";
  import { createDraggable, createDroppable } from "@dnd-kit/svelte";
  import { untrack } from "svelte";
  import { hasListThumbnail, isPreviewable } from "../fileKind";
  import { formatBytes, formatDatetime } from "../format";
  import { session } from "../session.svelte";
  import type { FileView } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";
  import FileTypeIcon from "./FileTypeIcon.svelte";

  const PREVIEW_POLL_MS = 3000;
  const PREVIEW_MAX_ATTEMPTS = 5;

  let {
    entry,
    rootId,
    name,
    columns,
    selected,
    onToggle,
    onOpen,
    onPreview,
    onDownload,
    onCopy,
    onDelete,
  }: {
    entry: FileView;
    rootId: string;
    name: string;
    columns: string;
    selected: boolean;
    onToggle: () => void;
    onOpen: () => void;
    onPreview: () => void;
    onDownload: () => void;
    onCopy: () => void;
    onDelete: () => void;
  } = $props();

  const file = $derived(entry.file);

  let shareOpen = $state(false);
  let shareMenu: HTMLDivElement | undefined = $state();

  function closeShare() {
    shareOpen = false;
  }

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
  const showThumbnail = $derived(hasListThumbnail(entry));

  function onRowDoubleClick() {
    if (previewable) {
      onPreview();
    }
  }

  let previewUrl = $state<string | null>(null);

  $effect(() => {
    const fileId = entry.file.id;
    const hasPreview = entry.metadata?.preview != null;
    const thumbnailable = hasListThumbnail(entry);

    previewUrl = null;

    if (entry.file.kind !== "file" || (!hasPreview && !thumbnailable)) {
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;
    let attempts = 0;
    let interval: ReturnType<typeof setInterval> | undefined;

    function stop() {
      if (interval !== undefined) {
        clearInterval(interval);
        interval = undefined;
      }
    }

    function load() {
      attempts += 1;

      session.client
        .preview(rootId, fileId)
        .then((blob) => {
          if (cancelled) {
            return;
          }
          stop();
          objectUrl = URL.createObjectURL(blob);
          previewUrl = objectUrl;
        })
        .catch(() => {
          if (cancelled || attempts >= PREVIEW_MAX_ATTEMPTS) {
            stop();
          }
        });
    }

    load();

    if (!hasPreview) {
      interval = setInterval(load, PREVIEW_POLL_MS);
    }

    return () => {
      cancelled = true;
      stop();
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  });
</script>

<svelte:window
  onclick={(event) => {
    if (!shareOpen || !shareMenu) {
      return;
    }
    if (!shareMenu.contains(event.target as Node)) {
      closeShare();
    }
  }}
  onkeydown={(event) => {
    if (event.key === "Escape") {
      closeShare();
    }
  }}
/>

<li
  {@attach draggable.attach}
  {@attach directory ? droppable.attach : undefined}
  class={cn(
    columns,
    "min-h-11 cursor-grab py-1 hover:bg-accent/60 active:cursor-grabbing",
    selected && "bg-accent/40",
    draggable.isDragging && "opacity-40",
    directory &&
      droppable.isDropTarget &&
      "bg-primary/10 ring-1 ring-primary ring-inset",
  )}
  ondblclick={onRowDoubleClick}
>
  <input
    type="checkbox"
    class="size-4"
    data-no-drag
    aria-label="Select {name}"
    checked={selected}
    onchange={onToggle}
    ondblclick={(event) => event.stopPropagation()}
  />
  {#if file.kind === "directory"}
    <button
      type="button"
      class="flex min-h-9 w-full min-w-0 items-center gap-3 overflow-hidden text-left text-sm font-medium"
      onclick={onOpen}
    >
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
      >
        <Folder size={18} strokeWidth={1.75} />
      </span>
      <span class="truncate" title={name}>{name}</span>
    </button>
  {:else if previewable}
    <div
      class="flex min-h-9 w-full min-w-0 items-center gap-3 overflow-hidden text-left text-sm"
    >
      <button
        type="button"
        class="group/preview relative size-8 shrink-0 cursor-pointer overflow-hidden rounded-md ring-1 ring-border/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        data-no-drag
        aria-label="Preview {name}"
        title="Preview"
        onclick={onPreview}
      >
        {#if showThumbnail && previewUrl}
          <img
            src={previewUrl}
            alt=""
            width="32"
            height="32"
            class="size-full object-cover shadow-sm"
          />
        {:else}
          <span
            class="flex size-full items-center justify-center bg-muted text-muted-foreground"
          >
            <FileTypeIcon fileType={file.fileType} size={16} />
          </span>
        {/if}
        <span
          class="absolute inset-0 flex items-center justify-center bg-black/55 text-white opacity-0 transition-opacity group-hover/preview:opacity-100 group-focus-visible/preview:opacity-100"
        >
          <Eye size={16} />
        </span>
      </button>
      <span class="truncate font-medium" title={name}>{name}</span>
    </div>
  {:else}
    <div
      class="flex min-h-9 w-full min-w-0 items-center gap-3 overflow-hidden text-left text-sm"
    >
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground ring-1 ring-border/60"
      >
        <FileTypeIcon fileType={file.fileType} size={16} />
      </span>
      <span class="truncate font-medium" title={name}>{name}</span>
    </div>
  {/if}
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
    data-no-drag
    ondblclick={(event) => event.stopPropagation()}
  >
    {#if file.kind === "file"}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        class="size-8 px-0 text-muted-foreground"
        aria-label="Download {name}"
        onclick={onDownload}
      >
        <Download size={16} />
      </Button>
    {/if}
    <div class="relative" bind:this={shareMenu}>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        class="size-8 px-0 text-muted-foreground"
        aria-label="Share {name}"
        aria-haspopup="menu"
        aria-expanded={shareOpen}
        onclick={(event) => {
          event.stopPropagation();
          shareOpen = !shareOpen;
        }}
      >
        <Share2 size={16} />
      </Button>
      {#if shareOpen}
        <div
          role="menu"
          class="absolute top-full right-0 z-50 mt-1 w-40 overflow-hidden rounded-xl border bg-card shadow-lg"
        >
          <div class="p-1">
            <button
              type="button"
              role="menuitem"
              class="flex min-h-11 w-full items-center rounded-md px-3 text-left text-sm hover:bg-accent md:min-h-9"
              onclick={() => {
                closeShare();
                onCopy();
              }}
            >
              Copy to…
            </button>
          </div>
        </div>
      {/if}
    </div>
    <Button
      type="button"
      variant="ghost"
      size="sm"
      class="size-8 px-0 text-muted-foreground hover:text-destructive"
      aria-label="Delete {name}"
      onclick={onDelete}
    >
      <Trash size={16} />
    </Button>
  </div>
</li>
