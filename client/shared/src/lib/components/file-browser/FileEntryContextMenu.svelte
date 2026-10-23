<script lang="ts">
  import {
    ChevronRight,
    Download,
    FolderInput,
    Link as LinkIcon,
    Link2Off,
    Pencil,
    Share2,
    Trash,
  } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import ContextMenu from "../ui/menu/context-menu.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import MenuSeparator from "../ui/menu/menu-separator.svelte";

  let {
    open = $bindable(false),
    x,
    y,
    bulk = false,
    shared,
    canWrite = true,
    canShare = true,
    canCopy = true,
    onDownload,
    onCopy,
    onShare,
    onUnshare,
    onCopyLink,
    onMove,
    onRename,
    onDelete,
  }: {
    open?: boolean;
    x: number;
    y: number;
    bulk?: boolean;
    shared: boolean;
    canWrite?: boolean;
    canShare?: boolean;
    canCopy?: boolean;
    onDownload: () => void;
    onCopy: () => void;
    onShare: () => void | Promise<void>;
    onUnshare: () => void | Promise<void>;
    onCopyLink: () => void | Promise<void>;
    onMove: () => void;
    onRename?: () => void;
    onDelete: () => void;
  } = $props();

  const writeDeniedTitle = "You do not have write access";
  const shareDeniedTitle = "You do not have share access";

  function copyMenuTitle(): string {
    if (canCopy) {
      return "Copy";
    }
    return "There are no writable folders to copy to";
  }

  let shareOpen = $state(false);
  let shareCloseTimer: ReturnType<typeof setTimeout> | undefined;

  function clearShareCloseTimer() {
    if (shareCloseTimer !== undefined) {
      clearTimeout(shareCloseTimer);
      shareCloseTimer = undefined;
    }
  }

  function scheduleShareClose() {
    clearShareCloseTimer();
    shareCloseTimer = setTimeout(() => {
      shareCloseTimer = undefined;
      shareOpen = false;
    }, 120);
  }

  // Reset flyout when the context menu closes (not derivable from open alone).
  $effect(() => {
    if (!open) {
      clearShareCloseTimer();
      shareOpen = false;
    }
  });

  onDestroy(() => {
    clearShareCloseTimer();
  });
</script>

<ContextMenu bind:open {x} {y} class="w-44">
  <MenuItem onclick={onDownload}>
    <Download size={16} />
    {bulk ? "Download as zip" : "Download"}
  </MenuItem>
  {#if onRename && !bulk}
    <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
      <MenuItem
        disabled={!canWrite}
        aria-label={canWrite ? "Rename" : writeDeniedTitle}
        onclick={onRename}
      >
        <Pencil size={16} />
        Rename
      </MenuItem>
    </span>
  {/if}
  <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
    <MenuItem
      disabled={!canWrite}
      aria-label={canWrite ? "Move" : writeDeniedTitle}
      onclick={onMove}
    >
      <FolderInput size={16} />
      Move
    </MenuItem>
  </span>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="relative"
    onpointerenter={() => {
      clearShareCloseTimer();
      shareOpen = true;
    }}
    onpointerleave={() => {
      scheduleShareClose();
    }}
  >
    <MenuItem closeOnSelect={false}>
      <Share2 size={16} />
      Share
      <ChevronRight size={14} class="ml-auto" />
    </MenuItem>
    {#if shareOpen}
      <div
        role="menu"
        class="absolute top-0 left-full z-50 ml-0.5 w-44 rounded-xl border bg-card p-1 shadow-lg"
      >
        <span class="block w-full" title={canCopy ? undefined : copyMenuTitle()}>
          <MenuItem
            disabled={!canCopy}
            aria-label={copyMenuTitle()}
            onclick={onCopy}
          >
            <Share2 size={16} />
            Copy
          </MenuItem>
        </span>
        {#if shared}
          {#if !bulk}
            <MenuItem onclick={onCopyLink}>
              <LinkIcon size={16} />
              Copy public link
            </MenuItem>
          {/if}
          <span
            class="block w-full"
            title={canShare ? undefined : shareDeniedTitle}
          >
            <MenuItem
              disabled={!canShare}
              aria-label={canShare ? "Stop sharing" : shareDeniedTitle}
              onclick={onUnshare}
            >
              <Link2Off size={16} />
              Stop sharing
            </MenuItem>
          </span>
        {:else}
          <span
            class="block w-full"
            title={canShare ? undefined : shareDeniedTitle}
          >
            <MenuItem
              disabled={!canShare}
              aria-label={canShare ? "Create public link" : shareDeniedTitle}
              onclick={onShare}
            >
              <LinkIcon size={16} />
              Create public link
            </MenuItem>
          </span>
        {/if}
      </div>
    {/if}
  </div>
  <MenuSeparator />
  <span class="block w-full" title={canWrite ? undefined : writeDeniedTitle}>
    <MenuItem
      class="text-destructive hover:text-destructive [&_svg]:text-destructive"
      disabled={!canWrite}
      aria-label={canWrite ? "Delete" : writeDeniedTitle}
      onclick={onDelete}
    >
      <Trash size={16} />
      Delete
    </MenuItem>
  </span>
</ContextMenu>
