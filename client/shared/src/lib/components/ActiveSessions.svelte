<script lang="ts">
  import { onMount } from "svelte";
  import { Monitor, Smartphone } from "@lucide/svelte";
  import { formatDatetime } from "../format";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { ActiveSessionView } from "../types";
  import Button from "./ui/button/button.svelte";
  import ConfirmDialog from "./ui/confirm-dialog/confirm-dialog.svelte";

  let sessions = $state.raw<ActiveSessionView[]>([]);
  let loading = $state(true);
  let busyAll = $state(false);
  let busyId = $state<string | null>(null);
  let confirmOthers = $state(false);

  let hasOtherSessions = $derived(sessions.some((s) => !s.current));
  let anyBusy = $derived(busyAll || busyId !== null);

  onMount(() => {
    void loadSessions();
  });

  async function loadSessions() {
    loading = true;
    try {
      sessions = await session.client.listSessions();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load sessions",
      );
    } finally {
      loading = false;
    }
  }

  async function expireOne(id: string) {
    if (anyBusy) {
      return;
    }

    busyId = id;
    try {
      await session.client.expireSession(id);
      sessions = sessions.filter((s) => s.id !== id);
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not sign out session",
      );
      await loadSessions();
    } finally {
      busyId = null;
    }
  }

  async function expireOthers() {
    if (anyBusy || !hasOtherSessions) {
      return;
    }

    busyAll = true;
    try {
      await session.client.expireOtherSessions();
      sessions = sessions.filter((s) => s.current);
      confirmOthers = false;
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not sign out other sessions",
      );
      await loadSessions();
    } finally {
      busyAll = false;
    }
  }

  function isMobileUa(ua: string): boolean {
    return /Mobile|Android|iPhone|iPad|iPod|webOS|BlackBerry|IEMobile|Opera Mini/i.test(
      ua,
    );
  }

  function detectBrowser(ua: string): string | null {
    if (/Edg(?:e|A|iOS)?\//i.test(ua)) {
      return "Edge";
    }
    if (/OPR\/|Opera\//i.test(ua)) {
      return "Opera";
    }
    if (/Chrome\/|CriOS\//i.test(ua)) {
      return "Chrome";
    }
    if (/Firefox\/|FxiOS\//i.test(ua)) {
      return "Firefox";
    }
    if (/Safari\//i.test(ua) && !/Chromium\//i.test(ua)) {
      return "Safari";
    }
    return null;
  }

  function detectOs(ua: string): string | null {
    if (/iPhone|iPad|iPod/i.test(ua) || (/Macintosh/i.test(ua) && /Mobile/i.test(ua))) {
      return "iOS";
    }
    if (/Android/i.test(ua)) {
      return "Android";
    }
    if (/Mac OS X|Macintosh/i.test(ua)) {
      return "macOS";
    }
    if (/Windows/i.test(ua)) {
      return "Windows";
    }
    if (/Linux/i.test(ua)) {
      return "Linux";
    }
    return null;
  }

  function deviceLabel(ua: string | null): string {
    if (!ua?.trim()) {
      return "Unknown device";
    }

    if (/nimbus|tauri/i.test(ua)) {
      return "Nimbus";
    }

    const browser = detectBrowser(ua);
    const os = detectOs(ua);

    if (browser && os) {
      return `${browser} on ${os}`;
    }
    if (browser) {
      return browser;
    }
    if (os) {
      return os;
    }

    return ua.length > 48 ? `${ua.slice(0, 45)}…` : ua;
  }

  function shortId(id: string): string {
    if (id.length <= 12) {
      return id;
    }
    return `${id.slice(0, 8)}…${id.slice(-4)}`;
  }

  async function copyId(id: string) {
    try {
      await navigator.clipboard.writeText(id);
    } catch {
      // Ignore clipboard failures; ID is still visible in title.
    }
  }
</script>

<section class="rounded-xl border bg-card p-4 sm:p-5">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <h3 class="min-w-0 text-sm font-semibold tracking-tight">Sessions</h3>
    <Button
      type="button"
      variant="outline"
      size="sm"
      class="shrink-0 border-destructive/30 text-destructive hover:bg-destructive/10"
      disabled={loading || anyBusy || !hasOtherSessions}
      onclick={() => (confirmOthers = true)}
    >
      Sign out others
    </Button>
  </div>

  {#if loading}
    <p class="mt-4 text-sm text-muted-foreground">Loading sessions…</p>
  {:else if sessions.length === 0}
    <p class="mt-4 text-sm text-muted-foreground">No active sessions.</p>
  {:else}
    <ul class="mt-4 space-y-2">
      {#each sessions as s (s.id)}
        {@const label = deviceLabel(s.userAgent)}
        {@const mobile = s.userAgent ? isMobileUa(s.userAgent) : false}
        <li
          class="flex flex-col gap-3 rounded-lg border bg-muted/15 p-3 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="flex min-w-0 flex-1 gap-3">
            <div
              class="flex size-9 shrink-0 items-center justify-center rounded-md border bg-background text-muted-foreground"
              aria-hidden="true"
            >
              {#if mobile}
                <Smartphone size={16} />
              {:else}
                <Monitor size={16} />
              {/if}
            </div>
            <div class="min-w-0 space-y-1.5">
              <div class="flex flex-wrap items-center gap-2">
                <p
                  class="truncate text-sm font-medium tracking-tight"
                  title={s.userAgent ?? undefined}
                >
                  {label}
                </p>
                {#if s.current}
                  <span
                    class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[11px] leading-none font-medium text-primary"
                  >
                    This device
                  </span>
                {/if}
              </div>
              <dl
                class="grid gap-x-4 gap-y-0.5 text-xs text-muted-foreground sm:grid-cols-2"
              >
                <div class="flex min-w-0 gap-1.5">
                  <dt class="shrink-0">IP</dt>
                  <dd class="truncate tabular-nums">
                    {s.remoteAddress ?? "-"}
                  </dd>
                </div>
                <div class="flex min-w-0 gap-1.5">
                  <dt class="shrink-0">Session</dt>
                  <dd class="min-w-0">
                    <button
                      type="button"
                      class="max-w-full truncate font-mono tabular-nums hover:text-foreground"
                      title={s.id}
                      onclick={() => void copyId(s.id)}
                    >
                      {shortId(s.id)}
                    </button>
                  </dd>
                </div>
                <div class="flex min-w-0 gap-1.5">
                  <dt class="shrink-0">Signed in</dt>
                  <dd class="truncate tabular-nums">
                    {formatDatetime(s.createdAt)}
                  </dd>
                </div>
                <div class="flex min-w-0 gap-1.5">
                  <dt class="shrink-0">Expires</dt>
                  <dd class="truncate tabular-nums">
                    {s.expiresAt ? formatDatetime(s.expiresAt) : "Never"}
                  </dd>
                </div>
              </dl>
            </div>
          </div>

          {#if !s.current}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="shrink-0 self-start"
              disabled={anyBusy}
              onclick={() => void expireOne(s.id)}
            >
              {busyId === s.id ? "Signing out…" : "Sign out"}
            </Button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<ConfirmDialog
  open={confirmOthers}
  title="Sign out other sessions?"
  description="All other devices will be signed out. This device stays signed in."
  confirmLabel="Sign out others"
  busy={busyAll}
  onCancel={() => {
    if (!busyAll) {
      confirmOthers = false;
    }
  }}
  onConfirm={() => void expireOthers()}
/>
