<script lang="ts">
  import { Download, File as FileIcon, Folder, Trash } from "@lucide/svelte";
  import { formatBytes, formatDatetime } from "../format";
  import type { FileEntry } from "../types";
  import { cn } from "../utils";
  import Button from "./ui/button/button.svelte";

  let {
    entry,
    name,
    columns,
    selected,
    onToggle,
    onOpen,
    onDownload,
    onDelete,
  }: {
    entry: FileEntry;
    name: string;
    columns: string;
    selected: boolean;
    onToggle: () => void;
    onOpen: () => void;
    onDownload: () => void;
    onDelete: () => void;
  } = $props();
</script>

<li
  class={cn(
    columns,
    "min-h-11 py-1 hover:bg-accent/60",
    selected && "bg-accent/40",
  )}
>
  <input
    type="checkbox"
    class="size-4"
    aria-label="Select {name}"
    checked={selected}
    onchange={onToggle}
  />
  {#if entry.kind === "directory"}
    <button
      type="button"
      class="flex min-h-11 w-full min-w-0 items-center gap-2.5 overflow-hidden text-left text-sm font-medium"
      onclick={onOpen}
    >
      <Folder size={18} class="shrink-0 text-primary" />
      <span class="truncate" title={name}>{name}</span>
    </button>
  {:else}
    <div
      class="flex min-h-11 w-full min-w-0 items-center gap-2.5 overflow-hidden text-left text-sm"
    >
      <FileIcon size={18} class="shrink-0 text-muted-foreground" />
      <span class="truncate" title={name}>{name}</span>
    </div>
  {/if}
  <span
    class="hidden min-w-0 overflow-hidden text-right text-sm text-muted-foreground tabular-nums @lg:block"
  >
    {entry.kind === "directory" ? "—" : formatBytes(entry.sizeBytes)}
  </span>
  <span class="hidden min-w-0 truncate text-sm text-muted-foreground @4xl:block">
    {formatDatetime(entry.createdAt)}
  </span>
  <span class="hidden min-w-0 truncate text-sm text-muted-foreground @2xl:block">
    {formatDatetime(entry.updatedAt)}
  </span>
  <div class="flex items-center justify-end gap-4">
    {#if entry.kind === "file"}
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
