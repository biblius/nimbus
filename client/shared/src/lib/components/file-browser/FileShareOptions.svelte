<script lang="ts">
  import { Copy, Download, Eye } from "@lucide/svelte";
  import { formatDatetime } from "../../format";
  import { toastError, toastSuccess } from "../../toast";
  import {
    type FileEntry,
    type ShareFile,
    type SharingMode,
  } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Dialog from "../ui/dialog/dialog.svelte";

  type ShareExpiryPreset = "never" | "1" | "7" | "30";

  const SHARE_EXPIRY_OPTIONS: {
    value: ShareExpiryPreset;
    label: string;
  }[] = [
    { value: "never", label: "Never" },
    { value: "1", label: "1 day" },
    { value: "7", label: "7 days" },
    { value: "30", label: "30 days" },
  ];

  let {
    entry,
    busy = false,
    onCancel,
    onShare,
    onUnshare,
  }: {
    entry: FileEntry;
    busy?: boolean;
    onCancel: () => void;
    /** Persist share settings (create or update). */
    onShare: (input: ShareFile) => void | Promise<void>;
    onUnshare: () => void | Promise<void>;
  } = $props();

  /** Null means follow `entry.sharingMode` (or download). Set when the user picks access. */
  let modeOverride = $state<SharingMode | null>(null);
  let shareExpiryPreset = $state<ShareExpiryPreset>("never");
  /** False until expiry is changed; preserves existing expiry on update. */
  let shareExpiryTouched = $state(false);

  const shareMode = $derived(modeOverride ?? entry.sharingMode ?? "download");
  const shareIsActive = $derived(entry.sharingMode != null);

  const publicLink = $derived(
    `${window.location.origin}/files/shared/${entry.id}`,
  );

  function shareExpiresAtPayload(): string | null {
    if (!shareExpiryTouched && shareIsActive) {
      return entry.shareExpiresAt ?? null;
    }

    if (shareExpiryPreset === "never") {
      return null;
    }

    const days = Number(shareExpiryPreset);
    return new Date(Date.now() + days * 24 * 60 * 60 * 1000).toISOString();
  }

  async function copyShareLink(message = "Public link copied") {
    try {
      await navigator.clipboard.writeText(publicLink);
      toastSuccess(message);
    } catch {
      toastError("Could not copy the public link");
    }
  }

  function submitShare() {
    void onShare({
      sharingMode: shareMode,
      shareExpiresAt: shareExpiresAtPayload(),
    });
  }
</script>

<Dialog
  open={true}
  title={shareIsActive ? "Manage link" : "Share link"}
  description="Anyone with the link can open this."
  {busy}
  class="max-w-lg"
  {onCancel}
>
  <div class="mt-4 space-y-4">
    <div class="space-y-1.5">
      <p class="text-sm font-medium">Link</p>
      <div
        class="flex items-center gap-2 rounded-md border border-border bg-muted/40 px-2.5 py-1.5"
      >
        <p
          class="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground"
          title={publicLink}
        >
          {publicLink}
        </p>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="shrink-0"
          aria-label="Copy public link"
          onclick={() => copyShareLink()}
          disabled={busy}
        >
          <Copy size={14} />
        </Button>
      </div>
      {#if !shareIsActive}
        <p class="text-xs text-muted-foreground">
          The link works after you create the share.
        </p>
      {/if}
    </div>

    <div class="space-y-2">
      <p class="text-sm font-medium">Access</p>
      <div role="radiogroup" aria-label="Access" class="grid grid-cols-2 gap-2">
        <button
          type="button"
          role="radio"
          aria-checked={shareMode === "view"}
          disabled={busy}
          class="flex flex-col items-start gap-1 rounded-md border px-3 py-2.5 text-left transition-colors disabled:opacity-50 {shareMode ===
          'view'
            ? 'border-primary bg-primary/5 text-foreground'
            : 'border-border text-foreground hover:bg-muted/50'}"
          onclick={() => (modeOverride = "view")}
        >
          <span class="flex items-center gap-2 text-sm font-medium">
            <Eye size={15} class="shrink-0 text-muted-foreground" />
            View
          </span>
          <span class="text-xs text-muted-foreground">
            Download controls hidden, content still available via preview
          </span>
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={shareMode === "download"}
          disabled={busy}
          class="flex flex-col items-start gap-1 rounded-md border px-3 py-2.5 text-left transition-colors disabled:opacity-50 {shareMode ===
          'download'
            ? 'border-primary bg-primary/5 text-foreground'
            : 'border-border text-foreground hover:bg-muted/50'}"
          onclick={() => (modeOverride = "download")}
        >
          <span class="flex items-center gap-2 text-sm font-medium">
            <Download size={15} class="shrink-0 text-muted-foreground" />
            Download
          </span>
          <span class="text-xs text-muted-foreground">
            Download controls shown
          </span>
        </button>
      </div>
    </div>

    <div class="space-y-2">
      <p class="text-sm font-medium">Expires</p>
      <div
        role="radiogroup"
        aria-label="Expires"
        class="flex flex-wrap gap-1.5"
      >
        {#each SHARE_EXPIRY_OPTIONS as option (option.value)}
          <button
            type="button"
            role="radio"
            aria-checked={shareExpiryPreset === option.value}
            disabled={busy}
            class="rounded-md border px-2.5 py-1 text-xs transition-colors disabled:opacity-50 {shareExpiryPreset ===
            option.value
              ? 'border-primary bg-muted text-foreground'
              : 'border-border text-muted-foreground hover:bg-muted/50'}"
            onclick={() => {
              shareExpiryPreset = option.value;
              shareExpiryTouched = true;
            }}
          >
            {option.label}
          </button>
        {/each}
      </div>
      {#if shareIsActive && entry.shareExpiresAt && !shareExpiryTouched}
        <p class="text-xs text-muted-foreground">
          Currently expires {formatDatetime(entry.shareExpiresAt)}
        </p>
      {/if}
    </div>
  </div>
  {#snippet footer()}
    <div class="flex w-full items-center justify-between gap-3">
      <div>
        {#if shareIsActive}
          <Button
            type="button"
            variant="ghost"
            class="text-destructive hover:bg-destructive/10 hover:text-destructive"
            onclick={() => onUnshare()}
            disabled={busy}
          >
            Stop sharing
          </Button>
        {/if}
      </div>
      <div class="flex gap-2">
        <Button
          type="button"
          variant="outline"
          onclick={onCancel}
          disabled={busy}
        >
          Cancel
        </Button>
        <Button type="button" onclick={submitShare} disabled={busy}>
          {shareIsActive ? "Save" : "Create link"}
        </Button>
      </div>
    </div>
  {/snippet}
</Dialog>
