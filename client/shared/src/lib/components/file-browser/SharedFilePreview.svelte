<script lang="ts">
  import {
    AppWindow,
    Archive,
    BookOpen,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    CirclePlay,
    Download,
    File as FileIcon,
    FileCode,
    FileText,
    Film,
    Image,
    Layers,
    Music,
    Type,
    X,
  } from "@lucide/svelte";
  import { fitMiddleEllipsis, formatBytes } from "../../format";
  import {
    FILE_FAMILY_LABELS,
    fileFamily,
    isAudioFile,
    isImageFile,
    isPdfFile,
    isTextFile,
    isVideoFile,
    isXlsxFile,
    isZipFile,
    parsePreviewFilter,
    type FileFamily,
    type PreviewFilter,
  } from "../../fileKind";
  import { session } from "../../session.svelte";
  import { kv } from "../../storage";
  import { toastError } from "../../toast";
  import type { SharedFileView } from "../../types";
  import { cn } from "../../utils";
  import AudioPreviewContent from "./preview/AudioPreviewContent.svelte";
  import ImagePreviewContent from "./preview/ImagePreviewContent.svelte";
  import VideoPreviewContent from "./preview/VideoPreviewContent.svelte";

  let {
    files: inputFiles,
    fileId,
    onClose,
    onSelect,
  }: {
    /** Previewable shared files in display order. */
    files: SharedFileView[];
    fileId: string;
    /** Omit when the preview is the whole page (single shared file). */
    onClose?: () => void;
    /** Omit when there is nothing to navigate between. */
    onSelect?: (fileId: string) => void;
  } = $props();

  const familyIcons: Record<FileFamily, typeof FileIcon> = {
    app: AppWindow,
    book: BookOpen,
    image: Image,
    video: Film,
    audio: Music,
    font: Type,
    doc: FileText,
    archive: Archive,
    text: FileCode,
    other: FileIcon,
  };

  let filter = $state<PreviewFilter>(
    parsePreviewFilter(kv.previewFilter.get()),
  );
  let autoplay = $state(kv.previewAutoplay.get());
  let filterOpen = $state(false);
  let filterMenu = $state<HTMLDivElement | null>(null);

  const files = $derived(
    filter === "all"
      ? inputFiles
      : inputFiles.filter(
          (entry) => fileFamily(entry.file.fileType) === filter,
        ),
  );

  let dialog = $state<HTMLDivElement | null>(null);

  const currentEntry = $derived(
    files.find((entry) => entry.file.id === fileId),
  );

  /** Families present in this preview set (for the dropdown). */
  const availableFamilies = $derived.by(() => {
    const seen = new Set<FileFamily>();
    const ordered: FileFamily[] = [];

    for (const entry of inputFiles) {
      const family = fileFamily(entry.file.fileType);
      if (!seen.has(family)) {
        seen.add(family);
        ordered.push(family);
      }
    }

    return ordered;
  });

  const showFilter = $derived(inputFiles.length > 1);

  const filterLabel = $derived(
    filter === "all" ? "All" : FILE_FAMILY_LABELS[filter],
  );
  const FilterIcon = $derived(filter === "all" ? Layers : familyIcons[filter]);

  const index = $derived(files.findIndex((entry) => entry.file.id === fileId));
  const current = $derived(index >= 0 ? files[index] : currentEntry);

  const hasPrev = $derived(index > 0);
  const hasNext = $derived(index >= 0 && index < files.length - 1);
  const isImage = $derived(current != null && isImageFile(current));
  const isVideo = $derived(current != null && isVideoFile(current));
  const isAudio = $derived(current != null && isAudioFile(current));
  const isText = $derived(current != null && isTextFile(current));
  const isPdf = $derived(current != null && isPdfFile(current));
  const isXlsx = $derived(current != null && isXlsxFile(current));
  const isZip = $derived(current != null && isZipFile(current));
  const fileName = $derived(current?.file.name ?? "");
  const canDownload = $derived(current?.file.sharingMode === "download");
  /** Full-file content URL for preview (view and download shares). */
  const contentUrl = $derived(
    current ? session.client.sharedContentUrl(current.file.id) : "",
  );
  /** Range-capable stream URL for video / audio. */
  const streamUrl = $derived(contentUrl);
  let displayName = $state("");

  // Fit the file name in the header by dynamically computing the width.
  function observeFileName(name: string) {
    return (node: HTMLElement) => {
      const sync = () => {
        const style = getComputedStyle(node);
        const font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
        displayName = fitMiddleEllipsis(name, node.clientWidth, font);
      };

      const observer = new ResizeObserver(sync);
      observer.observe(node);
      sync();

      return () => observer.disconnect();
    };
  }

  function toggleAutoplay() {
    autoplay = !autoplay;
    kv.previewAutoplay.set(autoplay);
  }

  function setFilter(next: PreviewFilter) {
    filter = next;
    kv.previewFilter.set(next);
    filterOpen = false;

    if (next === "all") {
      return;
    }

    const stillVisible = filteredFilesSome(next, fileId);

    if (!stillVisible) {
      const first = inputFiles.find(
        (entry) => fileFamily(entry.file.fileType) === next,
      );
      if (first) {
        onSelect?.(first.file.id);
      }
    }
  }

  function filteredFilesSome(family: FileFamily, id: string): boolean {
    return inputFiles.some(
      (entry) =>
        entry.file.id === id && fileFamily(entry.file.fileType) === family,
    );
  }

  function prev() {
    if (!hasPrev) {
      return;
    }

    onSelect?.(files[index - 1].file.id);
  }

  function next() {
    if (!hasNext) {
      return;
    }

    onSelect?.(files[index + 1].file.id);
  }

  function requestClose() {
    if (filterOpen) {
      filterOpen = false;
      return;
    }

    onClose?.();
  }

  async function download() {
    if (!current || !canDownload) {
      return;
    }

    try {
      await session.client.sharedDownload(current.file.id, current.file.name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${current.file.name}`,
      );
    }
  }

  // When opening a file that doesn't match the remembered filter, reset to All.
  $effect(() => {
    const id = fileId;

    const entry = inputFiles.find((item) => item.file.id === id);

    if (entry == null) {
      return;
    }

    const family = fileFamily(entry.file.fileType);

    if (filter !== "all" && filter !== family) {
      filter = "all";
      kv.previewFilter.set("all");
    }
  });

  $effect(() => {
    const currentIndex = index;
    const list = files;
    const menuOpen = filterOpen;

    if (!menuOpen) {
      dialog?.focus();
    }

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // Handles escape when filter dropdown is open
        if (menuOpen) {
          event.preventDefault();
          filterOpen = false;
          return;
        }

        onClose?.();

        return;
      }

      if (menuOpen) {
        return;
      }

      if (event.key === "ArrowLeft") {
        event.preventDefault();
        if (currentIndex > 0) {
          onSelect?.(list[currentIndex - 1].file.id);
        }
        return;
      }

      if (event.key === "ArrowRight") {
        event.preventDefault();
        if (currentIndex >= 0 && currentIndex < list.length - 1) {
          onSelect?.(list[currentIndex + 1].file.id);
        }
      }
    };

    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });
</script>

<svelte:window
  onclick={(event) => {
    if (!filterOpen || !filterMenu) {
      return;
    }
    if (!filterMenu.contains(event.target as Node)) {
      filterOpen = false;
    }
  }}
/>

<div
  bind:this={dialog}
  role="dialog"
  aria-modal="true"
  aria-label={current ? current.file.name : "File preview"}
  tabindex="-1"
  class="fixed inset-0 z-50 outline-none"
>
  {#if onClose}
    <button
      type="button"
      class="absolute inset-0 bg-zinc-950/92 backdrop-blur-sm"
      aria-label="Close"
      onclick={requestClose}
    ></button>
  {:else}
    <div class="absolute inset-0 bg-zinc-950/92" aria-hidden="true"></div>
  {/if}

  <header
    class="pointer-events-none absolute inset-x-0 top-0 z-20 grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)] items-start gap-3 bg-linear-to-b from-black/70 to-transparent px-4 pt-4 pb-16 sm:gap-4 sm:px-6 sm:pt-5"
  >
    <div
      class="pointer-events-auto flex min-w-0 items-center gap-2 justify-self-start"
    >
      <div class="relative" bind:this={filterMenu} class:hidden={!showFilter}>
        <button
          type="button"
          class={cn(
            "flex size-10 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors md:hidden",
            filterOpen
              ? "bg-white/18 ring-white/30"
              : "hover:bg-white/18 hover:ring-white/25",
          )}
          aria-label={filterOpen
            ? "Close preview options"
            : "Open preview options"}
          aria-haspopup="menu"
          aria-expanded={filterOpen}
          onclick={(event) => {
            event.stopPropagation();
            filterOpen = !filterOpen;
          }}
        >
          <svg
            viewBox="0 0 20 20"
            class="size-5"
            fill="none"
            stroke="currentColor"
            aria-hidden="true"
          >
            {#if filterOpen}
              <path
                d="M5 5l10 10M15 5L5 15"
                stroke-width="1.75"
                stroke-linecap="round"
              />
            {:else}
              <path
                d="M3 5h14M3 10h14M3 15h14"
                stroke-width="1.75"
                stroke-linecap="round"
              />
            {/if}
          </svg>
        </button>

        <button
          type="button"
          class={cn(
            "hidden h-10 items-center gap-2 rounded-full bg-white/10 pr-2.5 pl-3 text-sm font-medium text-white ring-1 ring-white/15 backdrop-blur-md transition-colors md:flex",
            filterOpen
              ? "bg-white/18 ring-white/30"
              : "hover:bg-white/18 hover:ring-white/25",
          )}
          aria-label="Filter by type"
          aria-haspopup="listbox"
          aria-expanded={filterOpen}
          onclick={(event) => {
            event.stopPropagation();
            filterOpen = !filterOpen;
          }}
        >
          <FilterIcon
            size={15}
            strokeWidth={1.75}
            class="shrink-0 text-white/70"
          />
          <span class="max-w-28 truncate tracking-tight">{filterLabel}</span>
          <ChevronDown
            size={14}
            strokeWidth={2}
            class={cn(
              "shrink-0 text-white/45 transition-transform duration-200",
              filterOpen && "rotate-180",
            )}
          />
        </button>

        {#if filterOpen}
          <div
            role="listbox"
            aria-label="File type filter"
            class="absolute top-full left-0 z-30 mt-2 min-w-52 overflow-hidden rounded-2xl bg-zinc-950/95 p-1.5 shadow-2xl ring-1 shadow-black/50 ring-white/12 backdrop-blur-xl"
          >
            <button
              type="button"
              role="option"
              aria-selected={filter === "all"}
              class={cn(
                "flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left text-sm transition-colors",
                filter === "all"
                  ? "bg-white/14 text-white"
                  : "text-white/75 hover:bg-white/8 hover:text-white",
              )}
              onclick={() => setFilter("all")}
            >
              <Layers
                size={15}
                strokeWidth={1.75}
                class="shrink-0 text-white/55"
              />
              <span class="flex-1 tracking-tight">All</span>
              {#if filter === "all"}
                <span
                  class="size-1.5 rounded-full bg-white/80"
                  aria-hidden="true"
                ></span>
              {/if}
            </button>

            {#each availableFamilies as family (family)}
              {@const Icon = familyIcons[family]}
              <button
                type="button"
                role="option"
                aria-selected={filter === family}
                class={cn(
                  "flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left text-sm transition-colors",
                  filter === family
                    ? "bg-white/14 text-white"
                    : "text-white/75 hover:bg-white/8 hover:text-white",
                )}
                onclick={() => setFilter(family)}
              >
                <Icon
                  size={15}
                  strokeWidth={1.75}
                  class="shrink-0 text-white/55"
                />
                <span class="flex-1 tracking-tight"
                  >{FILE_FAMILY_LABELS[family]}</span
                >
                {#if filter === family}
                  <span
                    class="size-1.5 rounded-full bg-white/80"
                    aria-hidden="true"
                  ></span>
                {/if}
              </button>
            {/each}

            {#if isVideo || isAudio}
              <div class="my-1 border-t border-white/10 md:hidden"></div>
              <button
                type="button"
                class={cn(
                  "flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left text-sm transition-colors md:hidden",
                  autoplay
                    ? "bg-white/14 text-white"
                    : "text-white/75 hover:bg-white/8 hover:text-white",
                )}
                aria-pressed={autoplay}
                aria-label="Toggle autoplay"
                onclick={(event) => {
                  event.stopPropagation();
                  toggleAutoplay();
                }}
              >
                <CirclePlay
                  size={15}
                  strokeWidth={1.75}
                  class="shrink-0 text-white/55"
                />
                <span class="flex-1 tracking-tight">Autoplay</span>
                {#if autoplay}
                  <span
                    class="size-1.5 rounded-full bg-white/80"
                    aria-hidden="true"
                  ></span>
                {/if}
              </button>
            {/if}
          </div>
        {/if}
      </div>

      {#if isVideo || isAudio}
        <button
          type="button"
          class={cn(
            "hidden h-10 items-center gap-2 rounded-full pr-3 pl-3 text-sm font-medium text-white ring-1 backdrop-blur-md transition-colors md:flex",
            autoplay
              ? "bg-white/18 ring-white/30"
              : "bg-white/10 ring-white/15 hover:bg-white/18 hover:ring-white/25",
          )}
          aria-pressed={autoplay}
          aria-label="Toggle autoplay"
          onclick={toggleAutoplay}
        >
          <CirclePlay
            size={15}
            strokeWidth={1.75}
            class="shrink-0 text-white/70"
          />
          <span class="tracking-tight">Autoplay</span>
        </button>
      {/if}
    </div>

    <div
      class="pointer-events-auto w-full max-w-md min-w-0 justify-self-center px-1 pt-1 text-center sm:max-w-lg"
    >
      <p
        class="overflow-hidden text-[15px] font-medium tracking-tight text-nowrap text-white"
        title={fileName}
        {@attach observeFileName(fileName)}
      >
        {displayName || fileName}
      </p>
      <p
        class="mt-0.5 flex items-center justify-center gap-2 text-xs text-white/55 tabular-nums"
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

    <div
      class="pointer-events-auto flex shrink-0 items-center gap-2 justify-self-end"
    >
      {#if canDownload}
        <button
          type="button"
          class="flex size-10 shrink-0 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18"
          aria-label="Download"
          title="Download"
          onclick={download}
        >
          <Download size={18} strokeWidth={2} />
        </button>
      {/if}
      {#if onClose}
        <button
          type="button"
          class="flex size-10 shrink-0 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18"
          aria-label="Close"
          onclick={requestClose}
        >
          <X size={18} strokeWidth={2} />
        </button>
      {/if}
    </div>
  </header>

  <div
    class={cn(
      "absolute inset-0 flex px-14 py-20 sm:px-20 sm:py-24",
      isText || isPdf || isXlsx || isZip
        ? "items-stretch justify-stretch"
        : "items-center justify-center",
    )}
  >
    {#if current && isImage}
      <ImagePreviewContent url={contentUrl} name={current.file.name} />
    {:else if current && isVideo}
      <VideoPreviewContent
        src={streamUrl}
        name={current.file.name}
        {autoplay}
        crossorigin="anonymous"
      />
    {:else if current && isAudio}
      <AudioPreviewContent
        src={streamUrl}
        name={current.file.name}
        {autoplay}
        crossorigin="anonymous"
      />
    {:else if current && isXlsx}
      {#await import("./preview/XlsxPreviewContent.svelte") then { default: XlsxPreviewContent }}
        <XlsxPreviewContent url={contentUrl} name={current.file.name} />
      {/await}
    {:else if current && isZip}
      {#await import("./preview/ZipPreviewContent.svelte") then { default: ZipPreviewContent }}
        <ZipPreviewContent url={contentUrl} name={current.file.name} />
      {/await}
    {:else if current && isText}
      {#await import("./preview/TextPreviewContent.svelte") then { default: TextPreviewContent }}
        <TextPreviewContent
          url={contentUrl}
          name={current.file.name}
          readonly
        />
      {/await}
    {:else if current && isPdf}
      {#await import("./preview/PdfPreviewContent.svelte") then { default: PdfPreviewContent }}
        <PdfPreviewContent url={contentUrl} name={current.file.name} />
      {/await}
    {/if}
  </div>

  <button
    type="button"
    class={cn(
      "absolute top-1/2 left-3 z-20 flex size-12 -translate-y-1/2 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-all sm:left-5",
      hasPrev
        ? "hover:bg-white/20 hover:ring-white/25"
        : "pointer-events-none opacity-0",
    )}
    aria-label="Previous file"
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
    aria-label="Next file"
    disabled={!hasNext}
    onclick={next}
  >
    <ChevronRight size={26} strokeWidth={1.75} />
  </button>
</div>
