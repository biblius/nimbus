<script lang="ts">
  import {
    getDocument,
    GlobalWorkerOptions,
    type PDFDocumentProxy,
    type RenderTask,
  } from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { session } from "../../../session.svelte";
  import { cn } from "../../../utils";

  GlobalWorkerOptions.workerSrc = workerUrl;

  const THUMB_WIDTH = 96;
  const DEFAULT_PAGE_ASPECT = "1 / 1.414";
  const LAZY_ROOT_MARGIN = "100% 0px";

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
  let pageAspect = $state(DEFAULT_PAGE_ASPECT);
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
    pageAspect = DEFAULT_PAGE_ASPECT;
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
          loadingTask.destroy();
          return;
        }

        try {
          const first = await doc.getPage(1);

          if (!cancelled) {
            const vp = first.getViewport({ scale: 1 });
            pageAspect = `${vp.width} / ${vp.height}`;
          }
        } catch {
          // Keep A4-ish fallback when page 1 metrics are unavailable.
        }

        if (cancelled) {
          loadingTask.destroy();
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
      loadingTask?.destroy();
      pdfDoc = null;
    };
  });

  function clearCanvas(canvas: HTMLCanvasElement) {
    canvas.width = 0;
    canvas.height = 0;
    canvas.style.width = "";
    canvas.style.height = "";
  }

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

  /**
   * Lazily rasterize when the reserved page box nears the scroll viewport;
   * cancel and release the bitmap when it leaves (with generous rootMargin).
   */
  function pageAttach(
    pageNum: number,
    doc: PDFDocumentProxy | null,
    width: number,
  ) {
    return (wrapper: HTMLElement) => {
      if (!doc || width <= 0) {
        return;
      }

      const canvas = wrapper.querySelector("canvas");

      if (!(canvas instanceof HTMLCanvasElement)) {
        return;
      }

      const scrollRoot = wrapper.closest("[data-pdf-scroll]");
      let destroyed = false;
      let renderTask: RenderTask | null = null;
      let paintId = 0;
      let visible = false;

      const cancelPaint = () => {
        paintId += 1;
        renderTask?.cancel();
        renderTask = null;
      };

      const paint = () => {
        const id = ++paintId;

        (async () => {
          try {
            const task = await renderPage(
              canvas,
              doc,
              pageNum,
              width,
              () => destroyed || id !== paintId,
            );

            if (!task || destroyed || id !== paintId) {
              task?.cancel();
              return;
            }

            renderTask = task;
            await task.promise;
          } catch {
            // Ignore cancel / destroy during navigation or unmount.
          } finally {
            if (id === paintId) {
              renderTask = null;
            }
          }
        })();
      };

      const observer = new IntersectionObserver(
        (entries) => {
          const entry = entries[entries.length - 1];

          if (!entry) {
            return;
          }

          if (entry.isIntersecting) {
            if (!visible) {
              visible = true;
              paint();
            }
          } else if (visible) {
            visible = false;
            cancelPaint();
            clearCanvas(canvas);
          }
        },
        {
          root: scrollRoot instanceof Element ? scrollRoot : null,
          rootMargin: LAZY_ROOT_MARGIN,
          threshold: 0,
        },
      );

      observer.observe(wrapper);

      return () => {
        destroyed = true;
        observer.disconnect();
        cancelPaint();
        clearCanvas(canvas);
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
        data-pdf-scroll
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
            <div
              class="w-full overflow-hidden rounded-sm bg-white shadow-sm"
              style:aspect-ratio={pageAspect}
              {@attach pageAttach(pageNum, pdfDoc, THUMB_WIDTH)}
            >
              <canvas class="block h-full w-full"></canvas>
            </div>
            <span class="text-[11px] text-white/55 tabular-nums">{pageNum}</span
            >
          </button>
        {/each}
      </aside>

      <div
        data-pdf-scroll
        class="min-w-0 flex-1 overflow-auto"
        aria-label={name}
        {@attach observeWidth}
        {@attach observePageVisibility(pageCount)}
      >
        <div class="mx-auto flex w-full flex-col items-center gap-4 py-1">
          {#each pageNumbers as pageNum (pageNum)}
            <div
              data-page={pageNum}
              class="w-full"
              style:aspect-ratio={pageAspect}
              {@attach pageAttach(pageNum, pdfDoc, containerWidth)}
            >
              <canvas
                class="mx-auto block bg-white shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)]"
              ></canvas>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>
