<script lang="ts">
  import { ChevronLeft, ChevronRight, X } from "@lucide/svelte";
  import { formatBytes } from "../format";
  import { isImageFile, isTextFile } from "../fileKind";
  import type { FileEntry, FileView } from "../types";
  import { cn } from "../utils";
  import ImagePreviewContent from "./preview/ImagePreviewContent.svelte";
  import TextPreviewContent from "./preview/TextPreviewContent.svelte";

  let {
    rootId,
    files,
    fileId,
    onClose,
    onSelect,
    onUpdated,
  }: {
    rootId: string;
    /** Previewable files in display order. */
    files: FileView[];
    fileId: string;
    onClose: () => void;
    onSelect: (fileId: string) => void;
    onUpdated?: (entry: FileEntry) => void;
  } = $props();

  const index = $derived(files.findIndex((entry) => entry.file.id === fileId));
  const current = $derived(files[index]);
  const hasPrev = $derived(index > 0);
  const hasNext = $derived(index >= 0 && index < files.length - 1);
  const isImage = $derived(current != null && isImageFile(current));
  const isText = $derived(current != null && isTextFile(current));

  let editing = $state(false);
  let dialog = $state<HTMLDivElement | null>(null);
  let textPreview = $state<{ cancelIfEditing: () => boolean } | null>(null);

  function prev() {
    if (!hasPrev || editing) {
      return;
    }
    onSelect(files[index - 1].file.id);
  }

  function next() {
    if (!hasNext || editing) {
      return;
    }
    onSelect(files[index + 1].file.id);
  }

  function requestClose() {
    if (textPreview?.cancelIfEditing()) {
      return;
    }
    onClose();
  }

  $effect(() => {
    const currentIndex = index;
    const list = files;
    const blockNav = editing;

    dialog?.focus();

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (blockNav) {
          event.preventDefault();
          textPreview?.cancelIfEditing();
          return;
        }
        onClose();
        return;
      }

      if (blockNav) {
        return;
      }

      if (event.key === "ArrowLeft") {
        event.preventDefault();
        if (currentIndex > 0) {
          onSelect(list[currentIndex - 1].file.id);
        }
        return;
      }

      if (event.key === "ArrowRight") {
        event.preventDefault();
        if (currentIndex >= 0 && currentIndex < list.length - 1) {
          onSelect(list[currentIndex + 1].file.id);
        }
      }
    };

    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });
</script>

<div
  bind:this={dialog}
  role="dialog"
  aria-modal="true"
  aria-label={current ? current.file.name : "File preview"}
  tabindex="-1"
  class="fixed inset-0 z-50 outline-none"
>
  <button
    type="button"
    class="absolute inset-0 bg-zinc-950/92 backdrop-blur-sm"
    aria-label="Close"
    onclick={requestClose}
  ></button>

  <header
    class="pointer-events-none absolute inset-x-0 top-0 z-20 flex items-start justify-between gap-4 bg-gradient-to-b from-black/70 to-transparent px-4 pt-4 pb-16 sm:px-6 sm:pt-5"
  >
    <div class="pointer-events-auto min-w-0 pt-1">
      <p class="truncate text-[15px] font-medium tracking-tight text-white">
        {current?.file.name ?? ""}
      </p>
      <p
        class="mt-0.5 flex items-center gap-2 text-xs text-white/55 tabular-nums"
      >
        {#if current}
          <span>{formatBytes(current.file.sizeBytes)}</span>
        {/if}
        {#if files.length > 1 && index >= 0}
          <span class="text-white/30" aria-hidden="true">·</span>
          <span>{index + 1} of {files.length}</span>
        {/if}
      </p>
    </div>

    <button
      type="button"
      class="pointer-events-auto flex size-10 shrink-0 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18"
      aria-label="Close"
      onclick={requestClose}
    >
      <X size={18} strokeWidth={2} />
    </button>
  </header>

  <div
    class={cn(
      "absolute inset-0 flex px-14 py-20 sm:px-20 sm:py-24",
      isText ? "items-stretch justify-stretch" : "items-center justify-center",
    )}
  >
    {#if current && isImage}
      <ImagePreviewContent
        {rootId}
        fileId={current.file.id}
        name={current.file.name}
      />
    {:else if current && isText}
      <TextPreviewContent
        bind:this={textPreview}
        {rootId}
        fileId={current.file.id}
        name={current.file.name}
        {onUpdated}
        bind:editing
      />
    {/if}
  </div>

  <button
    type="button"
    class={cn(
      "absolute top-1/2 left-3 z-20 flex size-12 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-all sm:left-5",
      hasPrev && !editing
        ? "hover:bg-white/20 hover:ring-white/25"
        : "pointer-events-none opacity-0",
    )}
    aria-label="Previous file"
    disabled={!hasPrev || editing}
    onclick={prev}
  >
    <ChevronLeft size={26} strokeWidth={1.75} />
  </button>

  <button
    type="button"
    class={cn(
      "absolute top-1/2 right-3 z-20 flex size-12 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-all sm:right-5",
      hasNext && !editing
        ? "hover:bg-white/20 hover:ring-white/25"
        : "pointer-events-none opacity-0",
    )}
    aria-label="Next file"
    disabled={!hasNext || editing}
    onclick={next}
  >
    <ChevronRight size={26} strokeWidth={1.75} />
  </button>
</div>
