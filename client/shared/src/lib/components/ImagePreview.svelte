<script lang="ts">
  import { ChevronLeft, ChevronRight, X } from "@lucide/svelte";
  import { formatBytes } from "../format";
  import { session } from "../session.svelte";
  import type { FileView } from "../types";
  import { cn } from "../utils";

  let {
    rootId,
    files,
    fileId,
    onClose,
    onSelect,
  }: {
    rootId: string;
    /** Image files in display order. */
    files: FileView[];
    fileId: string;
    onClose: () => void;
    onSelect: (fileId: string) => void;
  } = $props();

  const index = $derived(files.findIndex((entry) => entry.file.id === fileId));
  const current = $derived(index >= 0 ? files[index] : null);
  const hasPrev = $derived(index > 0);
  const hasNext = $derived(index >= 0 && index < files.length - 1);

  let imageUrl = $state<string | null>(null);
  let loading = $state(false);
  let error = $state("");
  let dialog = $state<HTMLDivElement | null>(null);

  function prev() {
    if (!hasPrev) {
      return;
    }
    onSelect(files[index - 1].file.id);
  }

  function next() {
    if (!hasNext) {
      return;
    }
    onSelect(files[index + 1].file.id);
  }

  $effect(() => {
    const currentIndex = index;
    const list = files;

    dialog?.focus();

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
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

  $effect(() => {
    const id = fileId;

    imageUrl = null;
    error = "";
    loading = true;

    let cancelled = false;
    let objectUrl: string | null = null;

    session.client
      .content(rootId, id)
      .then((blob) => {
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(blob);
        imageUrl = objectUrl;
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = "Could not load image";
        loading = false;
      });

    return () => {
      cancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  });
</script>

<div
  bind:this={dialog}
  role="dialog"
  aria-modal="true"
  aria-label={current ? current.file.name : "Image preview"}
  tabindex="-1"
  class="fixed inset-0 z-50 outline-none"
>
  <button
    type="button"
    class="absolute inset-0 bg-zinc-950/92 backdrop-blur-sm"
    aria-label="Close"
    onclick={onClose}
  ></button>

  <!-- Top chrome -->
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
      onclick={onClose}
    >
      <X size={18} strokeWidth={2} />
    </button>
  </header>

  <!-- Stage -->
  <div
    class="absolute inset-0 flex items-center justify-center px-14 py-20 sm:px-20 sm:py-24"
  >
    {#if loading}
      <div
        class="size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
        aria-label="Loading"
      ></div>
    {:else if error}
      <p class="text-sm text-red-300">{error}</p>
    {:else if imageUrl}
      <img
        src={imageUrl}
        alt={current?.file.name ?? ""}
        class="max-h-full max-w-full object-contain shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] select-none"
        draggable="false"
      />
    {/if}
  </div>

  <!-- Side controls -->
  <button
    type="button"
    class={cn(
      "absolute top-1/2 left-3 z-20 flex size-12 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-all sm:left-5",
      hasPrev
        ? "hover:bg-white/20 hover:ring-white/25"
        : "pointer-events-none opacity-0",
    )}
    aria-label="Previous image"
    disabled={!hasPrev}
    onclick={prev}
  >
    <ChevronLeft size={26} strokeWidth={1.75} />
  </button>

  <button
    type="button"
    class={cn(
      "absolute top-1/2 right-3 z-20 flex size-12 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-all sm:right-5",
      hasNext
        ? "hover:bg-white/20 hover:ring-white/25"
        : "pointer-events-none opacity-0",
    )}
    aria-label="Next image"
    disabled={!hasNext}
    onclick={next}
  >
    <ChevronRight size={26} strokeWidth={1.75} />
  </button>
</div>
