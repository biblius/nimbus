<script lang="ts">
  import { Folder } from "@lucide/svelte";
  import { FILE_ICON_CHIP_CLASS, fileIconChipStyle } from "../../fileKind";
  import { navigate } from "../../router";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { FileEntry, FileView, Root } from "../../types";
  import { cn } from "../../utils";
  import FileTypeIcon from "./FileTypeIcon.svelte";

  let {
    root,
    query,
    onNavigate,
  }: {
    root: Root;
    /** Non-empty search text from parent. */
    query: string;
    /**
     * Optional; if not provided use navigate().
     * `fileId` selects/highlights that file via `?file=` (same as activity links).
     */
    onNavigate?: (parentId: string | null, fileId?: string) => void;
  } = $props();

  let dirs = $state.raw<Record<string, FileEntry>>({});
  let results = $state.raw<FileView[]>([]);
  let loading = $state(false);
  let error = $state(false);

  function pathLabel(
    entry: FileEntry,
    map: Record<string, FileEntry>,
    rootName: string,
  ): string {
    const parts: string[] = [];
    let parentId = entry.parent;
    while (parentId) {
      const dir = map[parentId];
      if (!dir) break;
      parts.push(dir.name);
      parentId = dir.parent;
    }
    parts.reverse();
    return [rootName, ...parts].join(" / ");
  }

  function goTo(parentId: string | null, fileId?: string) {
    if (onNavigate) {
      onNavigate(parentId, fileId);
      return;
    }
    const search = fileId ? { file: fileId } : undefined;
    if (parentId) {
      navigate("/files/:root_id/dir/:parent_id", {
        params: { root_id: root.id, parent_id: parentId },
        search,
      });
      return;
    }
    navigate("/files/:root_id", {
      params: { root_id: root.id },
      search,
    });
  }

  function openResult(entry: FileView) {
    if (entry.file.kind === "directory") {
      goTo(entry.file.id);
      return;
    }
    goTo(entry.file.parent, entry.file.id);
  }

  // Load directory map for path labels whenever the root changes.
  $effect(() => {
    const rootId = root.id;
    let cancelled = false;

    session.client
      .descendants(rootId)
      .then((next) => {
        if (cancelled) return;
        dirs = next;
      })
      .catch((cause) => {
        if (cancelled) return;
        dirs = {};
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load folder paths",
        );
      });

    return () => {
      cancelled = true;
    };
  });

  // Debounced name search whenever query or root changes.
  $effect(() => {
    const rootId = root.id;
    const trimmed = query.trim();
    let cancelled = false;

    if (!trimmed) {
      results = [];
      loading = false;
      error = false;
      return;
    }

    loading = true;
    error = false;

    const timer = setTimeout(() => {
      session.client
        .search(rootId, trimmed)
        .then((next) => {
          if (cancelled) return;
          results = next;
          loading = false;
        })
        .catch((cause) => {
          if (cancelled) return;
          results = [];
          error = true;
          loading = false;
          toastError(
            cause instanceof TypeError
              ? "The server could not be reached"
              : "Could not search files",
          );
        });
    }, 250);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });
</script>

{#if loading && results.length === 0}
  <p class="px-3 py-8 text-center text-sm text-muted-foreground">
    Searching...
  </p>
{:else if error && results.length === 0}
  <p class="px-3 py-8 text-center text-sm text-muted-foreground">
    Search failed
  </p>
{:else if !query.trim()}
  <p class="px-3 py-8 text-center text-sm text-muted-foreground">
    Type to search
  </p>
{:else if results.length === 0}
  <p class="px-3 py-8 text-center text-sm text-muted-foreground">No matches</p>
{:else}
  <ul class="divide-y">
    {#each results as entry (entry.file.id)}
      {@const file = entry.file}
      {@const path = pathLabel(file, dirs, root.name)}
      <li>
        <button
          type="button"
          class="flex w-full min-w-0 items-center gap-2.5 px-3 py-2 text-left hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:outline-none"
          onclick={() => openResult(entry)}
        >
          {#if file.kind === "directory"}
            <span
              class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
            >
              <Folder size={20} strokeWidth={1.75} />
            </span>
          {:else}
            <span
              class={cn(
                "flex size-9 shrink-0 items-center justify-center rounded-lg ring-1",
                FILE_ICON_CHIP_CLASS,
              )}
              style={fileIconChipStyle(file.fileType)}
            >
              <FileTypeIcon fileType={file.fileType} size={20} />
            </span>
          {/if}
          <span class="min-w-0 flex-1">
            <span class="block truncate text-sm font-medium">{file.name}</span>
            <span class="block truncate text-xs text-muted-foreground"
              >{path}</span
            >
          </span>
        </button>
      </li>
    {/each}
  </ul>
{/if}
