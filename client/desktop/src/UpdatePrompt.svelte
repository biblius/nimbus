<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { relaunch } from "@tauri-apps/plugin-process";
  import {
    check,
    type DownloadEvent,
    type Update,
  } from "@tauri-apps/plugin-updater";
  import { Download } from "@lucide/svelte";
  import { toastError } from "@nimbus/shared/toast";
  import Button from "../../shared/src/lib/components/ui/button/button.svelte";
  import Dialog from "../../shared/src/lib/components/ui/dialog/dialog.svelte";

  const percentFormat = new Intl.NumberFormat(undefined, {
    style: "percent",
    maximumFractionDigits: 0,
  });

  let open = $state(false);
  let busy = $state(false);
  let dismissed = $state(false);
  /** Class instance — avoid deep $state proxy so methods stay intact. */
  let update = $state.raw<Update | null>(null);
  let currentVersion = $state("");
  let downloaded = $state(0);
  let contentLength = $state<number | null>(null);
  let finished = $state(false);

  const progress = $derived(
    finished
      ? 1
      : contentLength && contentLength > 0
        ? Math.min(1, downloaded / contentLength)
        : 0,
  );
  const percentLabel = $derived(percentFormat.format(progress));
  const statusText = $derived(
    finished
      ? "Restarting…"
      : contentLength
        ? `Downloading… ${percentLabel}`
        : "Preparing download…",
  );
  const versionPill = $derived.by(() => {
    if (!update) {
      return "";
    }
    if (currentVersion) {
      return `${currentVersion} → ${update.version}`;
    }
    return update.version;
  });
  const releaseNotes = $derived(update?.body?.trim() || "");

  onMount(() => {
    void checkForUpdate();
  });

  async function checkForUpdate() {
    try {
      const [found, appVersion] = await Promise.all([
        check(),
        getVersion().catch(() => ""),
      ]);

      if (!found || dismissed) {
        return;
      }

      update = found;
      currentVersion = found.currentVersion || appVersion;
      open = true;
    } catch (error) {
      // Common in dev when updater endpoints/pubkey are unset.
      console.warn("update check failed", error);
    }
  }

  function dismiss() {
    if (busy) {
      return;
    }
    open = false;
    dismissed = true;
    update = null;
  }

  async function install() {
    if (!update || busy) {
      return;
    }

    // Set busy before awaiting so the progress UI appears immediately.
    busy = true;
    downloaded = 0;
    contentLength = null;
    finished = false;

    try {
      await update.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength ?? null;
            downloaded = 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            break;
          case "Finished":
            finished = true;
            break;
        }
      });

      finished = true;
      await relaunch();
    } catch (error) {
      console.error("update install failed", error);
      toastError("Could not install update");
      busy = false;
      finished = false;
      downloaded = 0;
      contentLength = null;
    }
  }
</script>

<Dialog
  {open}
  title="Update available"
  description="A newer version of Nimbus is ready to install."
  {busy}
  onCancel={dismiss}
>
  <div class="mt-4 space-y-4">
    {#if versionPill}
      <span
        class="inline-flex items-center rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground tabular-nums ring-1 ring-border"
      >
        {versionPill}
      </span>
    {/if}

    {#if releaseNotes}
      <div
        class="max-h-40 overflow-y-auto rounded-md border border-border bg-muted/30 px-3 py-2"
      >
        <p class="text-sm whitespace-pre-wrap text-muted-foreground">
          {releaseNotes}
        </p>
      </div>
    {:else}
      <p class="text-sm text-muted-foreground">
        No release notes were included with this update.
      </p>
    {/if}

    {#if busy}
      <div class="space-y-1.5" role="status" aria-live="polite">
        <div class="flex items-center justify-between gap-3 text-sm">
          <span class="min-w-0 truncate text-muted-foreground"
            >{statusText}</span
          >
          {#if !finished}
            <span class="shrink-0 text-muted-foreground tabular-nums">
              {percentLabel}
            </span>
          {/if}
        </div>
        <div
          class="h-1.5 overflow-hidden rounded-full bg-muted"
          role="progressbar"
          aria-label={statusText}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progress * 100)}
        >
          <div
            class="h-full rounded-full bg-primary transition-[width] duration-150"
            style:width="{progress * 100}%"
          ></div>
        </div>
        <p class="text-xs text-muted-foreground">
          Keep Nimbus open until the update finishes.
        </p>
      </div>
    {/if}
  </div>

  {#snippet footer()}
    {#if !busy}
      <Button type="button" variant="outline" onclick={dismiss}>Later</Button>
      <Button type="button" onclick={install}>
        <Download size={16} aria-hidden="true" />
        Install update
      </Button>
    {/if}
  {/snippet}
</Dialog>
