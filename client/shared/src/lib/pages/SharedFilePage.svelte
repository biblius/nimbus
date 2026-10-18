<script lang="ts">
  import { CircleAlert, Download, Link2Off } from "@lucide/svelte";
  import { ApiError } from "../api";
  import SharedFileBrowser from "../components/file-browser/SharedFileBrowser.svelte";
  import SharedFilePreview from "../components/file-browser/SharedFilePreview.svelte";
  import Button from "../components/ui/button/button.svelte";
  import { isPreviewable } from "../fileKind";
  import { route } from "../router";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { SharedFileView } from "../types";

  type LoadState = "loading" | "ready" | "not-found" | "error";

  let entry = $state<SharedFileView | null>(null);
  let status = $state<LoadState>("loading");

  const fileId = $derived(route.params.file_id);
  const parentId = $derived(route.params.parent_id ?? null);
  const pageTitle = $derived(
    entry
      ? `${entry.file.name} · Nimbus`
      : status === "not-found"
        ? "Link unavailable · Nimbus"
        : status === "error"
          ? "Couldn't load share · Nimbus"
          : "Nimbus",
  );

  // Only reload the share root when the shared file id changes. Navigating
  // between directories in the same share keeps the loaded root.
  $effect(() => {
    const id = fileId;

    entry = null;
    status = "loading";

    let cancelled = false;

    session.client
      .sharedFile(id)
      .then((view) => {
        if (cancelled) {
          return;
        }

        entry = view;
        status = "ready";
      })
      .catch((cause) => {
        if (cancelled) {
          return;
        }

        status =
          cause instanceof ApiError &&
          (cause.status === 404 || cause.status === 403)
            ? "not-found"
            : "error";
      });

    return () => {
      cancelled = true;
    };
  });

  async function download(view: SharedFileView) {
    const name =
      view.file.kind === "directory"
        ? `${view.file.name}.zip`
        : view.file.name;

    try {
      await session.client.sharedDownload(view.file.id, name);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : `Could not download ${view.file.name}`,
      );
    }
  }
</script>

<svelte:head>
  <title>{pageTitle}</title>
</svelte:head>

{#if status === "ready" && entry && entry.file.kind === "file" && isPreviewable(entry)}
  <!-- Standalone preview; it brings its own header with name and download. -->
  <SharedFilePreview files={[entry]} fileId={entry.file.id} />
{:else}
  <div class="flex h-dvh min-h-0 flex-col overflow-hidden">
    <header
      class="flex h-14 shrink-0 items-center justify-between gap-3 border-b bg-card px-4 sm:px-6"
    >
      <div class="flex min-w-0 items-center gap-2">
        <span class="text-sm font-semibold tracking-tight">Nimbus</span>
        {#if entry}
          <span class="text-muted-foreground" aria-hidden="true">/</span>
          <h1 class="min-w-0 truncate text-sm font-medium">
            {entry.file.name}
          </h1>
        {/if}
      </div>
      {#if entry && entry.file.sharingMode === "download"}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onclick={() => download(entry!)}
        >
          <Download size={15} />
          Download
        </Button>
      {/if}
    </header>

    <main class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-4 sm:p-6">
      <div class="mx-auto flex min-h-0 w-full max-w-5xl flex-1 flex-col">
        {#if status === "loading"}
          <div
            class="flex flex-1 flex-col items-center justify-center gap-2 px-6 py-16 text-center"
          >
            <p class="text-sm text-muted-foreground">Loading…</p>
          </div>
        {:else if status === "not-found"}
          <div
            class="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-16 text-center"
          >
            <div
              class="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground"
              aria-hidden="true"
            >
              <Link2Off size={22} />
            </div>
            <div class="flex flex-col gap-1.5">
              <p class="text-sm font-medium">Link unavailable</p>
              <p class="max-w-sm text-sm text-muted-foreground">
                This share may have been removed, or the link is no longer
                valid.
              </p>
            </div>
          </div>
        {:else if status === "error"}
          <div
            class="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-16 text-center"
          >
            <div
              class="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground"
              aria-hidden="true"
            >
              <CircleAlert size={22} />
            </div>
            <div class="flex flex-col gap-1.5">
              <p class="text-sm font-medium">Couldn't load share</p>
              <p class="max-w-sm text-sm text-muted-foreground">
                Something went wrong while opening this link. Try again in a
                moment.
              </p>
            </div>
          </div>
        {:else if entry && entry.file.kind === "directory"}
          <SharedFileBrowser shareRoot={entry} {parentId} />
        {:else if entry}
          <p class="text-sm text-muted-foreground">
            {entry.file.sharingMode === "download"
              ? "This file cannot be previewed. Use the download button to save it."
              : "This file cannot be previewed."}
          </p>
        {/if}
      </div>
    </main>
  </div>
{/if}
