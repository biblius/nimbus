<script lang="ts">
  import {
    getDocument,
    GlobalWorkerOptions,
    type PDFDocumentProxy,
    type RenderTask,
  } from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { session } from "../../session.svelte";
  import { cn } from "../../utils";

  GlobalWorkerOptions.workerSrc = workerUrl;

  const THUMB_WIDTH = 96;

  let {
    rootId,
    fileId,
    name,
  }: {
    rootId: string;
    fileId: string;
    name: string;
  } = $props();

  let loading = $state(true);
  let error = $state("");
  let pageCount = $state(0);
  let currentPage = $state(1);
  let containerWidth = $state(0);
  let pdfDoc = $state.raw<PDFDocumentProxy | null>(null);

  let mainPane: HTMLElement | null = null;

  const pageNumbers = $derived(
    Array.from({ length: pageCount }, (_, i) => i + 1),
  );

  $effect(() => {
    const id = fileId;

    loading = true;
    error = "";
    pageCount = 0;
    currentPage = 1;
    pdfDoc = null;

    let cancelled = false;
    let loadingTask: ReturnType<typeof getDocument> | null = null;
    let doc: PDFDocumentProxy | null = null;

    session.client
      .content(rootId, id)
      .then(async (blob) => {
        if (cancelled) {
          return;
        }

        const data = new Uint8Array(await blob.arrayBuffer());

        if (cancelled) {
          return;
        }

        loadingTask = getDocument({ data });
        doc = await loadingTask.promise;

        if (cancelled) {
          void doc.destroy();
          return;
        }

        pdfDoc = doc;
        pageCount = doc.numPages;
        currentPage = 1;
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = "Could not load PDF";
        loading = false;
      });

    return () => {
      cancelled = true;
      void loadingTask?.destroy();
      void doc?.destroy();
      pdfDoc = null;
    };
  });

  async function renderPage(
    canvas: HTMLCanvasElement,
    doc: PDFDocumentProxy,
    pageNum: number,
    width: number,
    isCancelled: () => boolean,
  ): Promise<RenderTask | null> {
    const page = await doc.getPage(pageNum);

    if (isCancelled()) {
      return null;
    }

    const base = page.getViewport({ scale: 1 });
    const scale = width / base.width;
    const viewport = page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;

    canvas.width = Math.floor(viewport.width * dpr);
    canvas.height = Math.floor(viewport.height * dpr);
    canvas.style.width = `${Math.floor(viewport.width)}px`;
    canvas.style.height = `${Math.floor(viewport.height)}px`;

    const ctx = canvas.getContext("2d");

    if (!ctx || isCancelled()) {
      return null;
    }

    return page.render({
      canvasContext: ctx,
      canvas,
      viewport,
      transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
    });
  }

  // Attached to the PDF container div.
  // Observes changed box width.
  // Sets `mainPane` to the node.
  function observeWidth(node: HTMLDivElement) {
    mainPane = node;

    const observer = new ResizeObserver((entries) => {
      if (entries[0]) {
        containerWidth = entries[0].contentRect.width;
      }
    });

    observer.observe(node);

    containerWidth = node.clientWidth;

    return () => {
      if (mainPane === node) {
        mainPane = null;
      }
      observer.disconnect();
    };
  }

  // Observes the page visibility, i.e. the intersection of the
  // PDF container div with the canvases the main pages are rendered on.
  // Compares the intersection ratio of pages intersecting the container
  // (usually 2) and sets the `currentPage` variable to the one which wins.
  function observePageVisibility(count: number) {
    return (node: HTMLElement) => {
      if (count <= 0) {
        return;
      }

      const visibility = new Map<number, IntersectionObserverEntry>();

      const observer = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            // Set by `data-page`
            const page = Number((entry.target as HTMLElement).dataset.page);

            if (!Number.isFinite(page)) {
              continue;
            }

            if (entry.isIntersecting && entry.intersectionRatio > 0) {
              visibility.set(page, entry);
            } else {
              visibility.delete(page);
            }
          }

          if (visibility.size === 0) {
            return;
          }

          let bestPage = 0;
          let bestRatio = -1;
          let bestTop = Infinity;

          for (const [page, entry] of visibility) {
            const top = entry.boundingClientRect.top;

            if (
              entry.intersectionRatio > bestRatio ||
              (entry.intersectionRatio === bestRatio && top < bestTop)
            ) {
              bestRatio = entry.intersectionRatio;
              bestTop = top;
              bestPage = page;
            }
          }

          if (bestPage > 0) {
            currentPage = bestPage;
          }
        },
        {
          root: node,
          threshold: [0, 0.1, 0.25, 0.5, 0.75, 1],
        },
      );

      const observeAll = () => {
        for (const el of node.querySelectorAll<HTMLElement>("[data-page]")) {
          observer.observe(el);
        }
      };

      observeAll();

      const raf = requestAnimationFrame(observeAll);

      return () => {
        cancelAnimationFrame(raf);
        observer.disconnect();
        visibility.clear();
      };
    };
  }

  function pageAttach(
    pageNum: number,
    doc: PDFDocumentProxy | null,
    width: number,
  ) {
    return (canvas: HTMLCanvasElement) => {
      if (!doc || width <= 0) {
        return;
      }

      let cancelled = false;
      let renderTask: RenderTask | null = null;

      void (async () => {
        try {
          renderTask = await renderPage(
            canvas,
            doc,
            pageNum,
            width,
            () => cancelled,
          );

          if (!renderTask || cancelled) {
            renderTask?.cancel();
            return;
          }

          await renderTask.promise;
        } catch {
          // Ignore cancel / destroy during navigation or unmount.
        }
      })();

      return () => {
        cancelled = true;
        renderTask?.cancel();
      };
    };
  }

  function goToPage(pageNum: number) {
    currentPage = pageNum;

    const el = mainPane?.querySelector(`[data-page="${pageNum}"]`);

    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  /** Scroll the sidebar so the active thumb stays in view. */
  function keepThumbVisible(pageNum: number) {
    return (node: HTMLElement) => {
      $effect(() => {
        if (currentPage === pageNum) {
          node.scrollIntoView({ block: "nearest" });
        }
      });
    };
  }
</script>

<div class="flex h-full w-full flex-col gap-3">
  {#if loading}
    <div
      class="m-auto size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
      aria-label="Loading"
    ></div>
  {:else if error}
    <p class="m-auto text-sm text-red-300">{error}</p>
  {:else if pdfDoc && pageCount > 0}
    <div class="flex min-h-0 flex-1 gap-3">
      <aside
        class="hidden w-28 shrink-0 flex-col gap-2 overflow-y-auto rounded-xl bg-zinc-900/90 p-2 ring-1 ring-white/10 backdrop-blur-md sm:flex"
        aria-label="Page overview"
      >
        {#each pageNumbers as pageNum (pageNum)}
          <button
            type="button"
            class={cn(
              "flex w-full flex-col items-center gap-1.5 rounded-lg p-1.5 transition-colors",
              currentPage === pageNum
                ? "bg-white/14 ring-1 ring-white/40"
                : "ring-1 ring-transparent hover:bg-white/10",
            )}
            aria-label={`Go to page ${pageNum}`}
            aria-current={currentPage === pageNum ? "page" : undefined}
            {@attach keepThumbVisible(pageNum)}
            onclick={() => goToPage(pageNum)}
          >
            <canvas
              class="w-full rounded-sm bg-white shadow-sm"
              {@attach pageAttach(pageNum, pdfDoc, THUMB_WIDTH)}
            ></canvas>
            <span class="text-[11px] text-white/55 tabular-nums">{pageNum}</span
            >
          </button>
        {/each}
      </aside>

      <div
        class="min-w-0 flex-1 overflow-auto"
        aria-label={name}
        {@attach observeWidth}
        {@attach observePageVisibility(pageCount)}
      >
        <div class="mx-auto flex w-full flex-col items-center gap-4 py-1">
          {#each pageNumbers as pageNum (pageNum)}
            <div data-page={pageNum} class="w-full">
              <canvas
                class="mx-auto block bg-white shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)]"
                {@attach pageAttach(pageNum, pdfDoc, containerWidth)}
              ></canvas>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>
