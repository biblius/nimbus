<script lang="ts">
  import {
    getDocument,
    GlobalWorkerOptions,
    type PDFDocumentProxy,
    type RenderTask,
  } from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import { session } from "../../session.svelte";

  GlobalWorkerOptions.workerSrc = workerUrl;

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
  let containerWidth = $state(0);
  let pdfDoc = $state.raw<PDFDocumentProxy | null>(null);

  const pageNumbers = $derived(
    Array.from({ length: pageCount }, (_, i) => i + 1),
  );

  $effect(() => {
    const id = fileId;

    loading = true;
    error = "";
    pageCount = 0;
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

  function observeWidth(node: HTMLDivElement) {
    const observer = new ResizeObserver((entries) => {
      if (entries[0]) {
        containerWidth = entries[0].contentRect.width;
      }
    });

    observer.observe(node);

    containerWidth = node.clientWidth;

    return () => observer.disconnect();
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
          const page = await doc.getPage(pageNum);
          if (cancelled) {
            return;
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
          if (!ctx || cancelled) {
            return;
          }

          renderTask = page.render({
            canvasContext: ctx,
            canvas,
            viewport,
            transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
          });
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
</script>

<div class="flex h-full w-full flex-col">
  {#if loading}
    <div
      class="m-auto size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
      aria-label="Loading"
    ></div>
  {:else if error}
    <p class="m-auto text-sm text-red-300">{error}</p>
  {:else if pdfDoc && pageCount > 0}
    <div
      class="h-full w-full max-w-full overflow-auto"
      aria-label={name}
      {@attach observeWidth}
    >
      <div class="mx-auto flex w-full flex-col items-center gap-4 py-1">
        {#each pageNumbers as pageNum (pageNum)}
          <canvas
            class="bg-white shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)]"
            {@attach pageAttach(pageNum, pdfDoc, containerWidth)}
          ></canvas>
        {/each}
      </div>
    </div>
  {/if}
</div>
