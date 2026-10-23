<script lang="ts">
  import { ChevronRight, Folder, RotateCcw, Trash } from "@lucide/svelte";
  import {
    FILE_ICON_CHIP_CLASS,
    fileIconChipStyle,
    treeHasThumbnail,
  } from "../fileKind";
  import { session } from "../session.svelte";
  import type { FileTree, Root } from "../types";
  import { cn } from "../utils";
  import FileTypeIcon from "./file-browser/FileTypeIcon.svelte";
  import Button from "./ui/button/button.svelte";

  let {
    root,
    node,
    depth,
    expanded,
    canWrite = true,
    onToggle,
    onRestore,
    onDelete,
  }: {
    root: Root;
    node: FileTree;
    depth: number;
    expanded: boolean;
    /** False when the caller lacks WRITE on this root. */
    canWrite?: boolean;
    onToggle: () => void;
    onRestore: () => void;
    onDelete: () => void;
  } = $props();

  const writeDeniedTitle = "You do not have write access";

  const hasChildren = $derived(node.children.length > 0);
  const showThumbnail = $derived(
    treeHasThumbnail(node.kind, node.fileType) &&
      node.metadata?.preview64 != null,
  );

  let previewUrl = $state<string | null>(null);

  $effect(() => {
    const fileId = node.id;
    const rootId = root.id;
    const preview = node.metadata?.preview64;
    const thumbnailable = treeHasThumbnail(node.kind, node.fileType);

    previewUrl = null;

    if (!thumbnailable || preview == null) {
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;

    session.client
      .icon(rootId, fileId, 64)
      .then((blob) => {
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(blob);
        previewUrl = objectUrl;
      })
      .catch(() => {
        /* trash thumbs are best-effort */
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
  class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 px-3 min-h-14 py-1.5 hover:bg-accent/60"
>
  <div
    class="flex min-w-0 items-center gap-2.5"
    style:padding-left="{depth * 1.25}rem"
  >
    {#if hasChildren}
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="shrink-0 text-muted-foreground hover:text-foreground"
        aria-expanded={expanded ? "true" : "false"}
        aria-label={expanded ? `Collapse ${node.name}` : `Expand ${node.name}`}
        onclick={onToggle}
      >
        <ChevronRight
          size={16}
          class={cn("transition-transform", expanded && "rotate-90")}
        />
      </Button>
    {:else}
      <span class="size-8 shrink-0"></span>
    {/if}

    {#if node.kind === "directory"}
      <span
        class="flex size-10 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary ring-1 ring-primary/15"
      >
        <Folder size={20} strokeWidth={1.75} />
      </span>
    {:else if showThumbnail && previewUrl}
      <img
        src={previewUrl}
        alt=""
        width="40"
        height="40"
        class="size-10 shrink-0 rounded-md object-cover shadow-sm ring-1 ring-border"
      />
    {:else}
      <span
        class={cn(
          "flex size-10 shrink-0 items-center justify-center rounded-md ring-1",
          FILE_ICON_CHIP_CLASS,
        )}
        style={fileIconChipStyle(node.fileType)}
      >
        <FileTypeIcon fileType={node.fileType} size={18} />
      </span>
    {/if}

    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium" title={node.name}>{node.name}</p>
      {#if node.kind === "directory" && hasChildren}
        <p class="truncate text-xs text-muted-foreground">
          {node.children.length}
          {node.children.length === 1 ? "item" : "items"}
        </p>
      {/if}
    </div>
  </div>

  <div class="flex items-center">
    <span class="inline-flex" title={canWrite ? undefined : writeDeniedTitle}>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        class="text-muted-foreground"
        disabled={!canWrite}
        aria-label={canWrite ? `Restore ${node.name}` : writeDeniedTitle}
        onclick={onRestore}
      >
        <RotateCcw size={16} />
        Restore
      </Button>
    </span>
    <span class="inline-flex" title={canWrite ? undefined : writeDeniedTitle}>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground hover:text-destructive"
        disabled={!canWrite}
        aria-label={canWrite
          ? `Permanently delete ${node.name}`
          : writeDeniedTitle}
        onclick={onDelete}
      >
        <Trash size={16} />
      </Button>
    </span>
  </div>
</div>
