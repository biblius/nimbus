<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    KeyRound,
    MailWarning,
    Pencil,
    ShieldAlert,
    ShieldCheck,
    Trash,
  } from "@lucide/svelte";
  import { ApiError, ValidationError } from "../api-errors";
  import ActiveSessions from "../components/ActiveSessions.svelte";
  import PanelShell from "../components/PanelShell.svelte";
  import Banner from "../components/ui/banner/banner.svelte";
  import Button from "../components/ui/button/button.svelte";
  import ConfirmDialog from "../components/ui/confirm-dialog/confirm-dialog.svelte";
  import Field from "../components/ui/field/field.svelte";
  import UserForm from "../components/user/UserForm.svelte";
  import UserInfo from "../components/user/UserInfo.svelte";
  import UserSidebar from "../components/UserSidebar.svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { Root, UserData, UserPasskeyView } from "../types";
  import { createPasskey } from "../webauthn";

  type OtpPanel = "idle" | "setup" | "disable";

  let roots = $state<Root[]>([]);
  let editing = $state(false);
  let otpPanel = $state<OtpPanel>("idle");
  let otpBusy = $state(false);
  let otpCode = $state("");
  let otpToken = $state<string | null>(null);
  let qrUrl = $state<string | null>(null);
  let otpValidation = $state<ValidationError | undefined>();
  let passkeys = $state<UserPasskeyView[]>([]);
  let passkeyBusy = $state(false);
  let deletePasskeyId = $state<number | null>(null);
  let deletingPasskey = $state(false);

  let passkeysEnabled = $derived(passkeys.length > 0);

  let missingEmail = $derived(
    Boolean(session.user && !session.user.email?.trim()),
  );
  let otpOff = $derived(Boolean(session.user && !session.user.otpEnabled));

  onMount(async () => {
    await Promise.all([
      (async () => {
        try {
          roots = await session.client.roots();
        } catch {
          toastError("Could not load files");
        }
      })(),
      loadPasskeys(),
    ]);
  });

  onDestroy(() => {
    if (qrUrl) {
      URL.revokeObjectURL(qrUrl);
    }
  });

  function clearOtpUi() {
    if (qrUrl) {
      URL.revokeObjectURL(qrUrl);
    }
    qrUrl = null;
    otpToken = null;
    otpCode = "";
    otpValidation = undefined;
    otpPanel = "idle";
  }

  function startEditing() {
    editing = true;
  }

  function stopEditing() {
    editing = false;
  }

  async function loadPasskeys() {
    try {
      passkeys = await session.client.listPasskeys();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load passkeys",
      );
    }
  }

  function isWebAuthnCancelled(cause: unknown): boolean {
    return (
      (cause instanceof DOMException || cause instanceof Error) &&
      (cause.name === "NotAllowedError" || cause.name === "AbortError")
    );
  }

  function formatPasskeyDate(iso: string): string {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) {
      return iso;
    }
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }

  async function removePasskey() {
    const id = deletePasskeyId;
    if (id === null || deletingPasskey) {
      return;
    }

    deletingPasskey = true;

    try {
      await session.client.deletePasskey(id);
      passkeys = passkeys.filter((p) => p.id !== id);
      deletePasskeyId = null;
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not remove passkey",
      );
    } finally {
      deletingPasskey = false;
    }
  }

  async function addPasskey() {
    if (passkeyBusy) {
      return;
    }

    passkeyBusy = true;

    try {
      const setup = await session.client.startPasskeySetup();
      const credential = await createPasskey(setup.challenge);
      await session.client.finishPasskeySetup(setup.token, credential);
      await loadPasskeys();
    } catch (cause) {
      if (isWebAuthnCancelled(cause)) {
        return;
      }

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not register passkey",
      );
    } finally {
      passkeyBusy = false;
    }
  }

  async function saveProfile(data: UserData) {
    const updated = await session.client.updateMe({
      username: data.username?.trim(),
      email: data.email || undefined,
      password: data.password || undefined,
      currentPassword: data.currentPassword || undefined,
    });

    session.user = updated;

    stopEditing();
  }

  async function startOtpSetup() {
    if (otpBusy || !session.user) {
      return;
    }

    otpBusy = true;
    otpValidation = undefined;

    try {
      const setup = await session.client.startOtpSetup();

      if (qrUrl) {
        URL.revokeObjectURL(qrUrl);
      }

      qrUrl = URL.createObjectURL(
        new Blob([setup.qrSvg], { type: "image/svg+xml" }),
      );

      otpToken = setup.token;
      otpCode = "";
      otpPanel = "setup";
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not start two-factor setup",
      );
    } finally {
      otpBusy = false;
    }
  }

  async function confirmOtpSetup(event: SubmitEvent) {
    event.preventDefault();

    if (!otpToken || otpBusy) {
      return;
    }

    otpBusy = true;
    otpValidation = undefined;

    try {
      await session.client.finishOtpSetup(otpToken, otpCode);
      session.user = await session.client.me();
      clearOtpUi();
    } catch (cause) {
      if (cause instanceof ValidationError) {
        otpValidation = cause;
        return;
      }

      // The setup token is one-shot; a wrong code consumes it, so reset the flow.
      clearOtpUi();
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ApiError && cause.status === 401
            ? "That code is incorrect. Scan a new QR code and try again."
            : "Could not enable two-factor authentication",
      );
    } finally {
      otpBusy = false;
    }
  }

  function beginDisableOtp() {
    otpCode = "";
    otpValidation = undefined;
    if (qrUrl) {
      URL.revokeObjectURL(qrUrl);
      qrUrl = null;
    }
    otpToken = null;
    otpPanel = "disable";
  }

  async function confirmDisableOtp(event: SubmitEvent) {
    event.preventDefault();

    if (otpBusy) {
      return;
    }

    otpBusy = true;
    otpValidation = undefined;

    try {
      await session.client.disableOtp(otpCode);
      session.user = await session.client.me();
      clearOtpUi();
    } catch (cause) {
      if (cause instanceof ValidationError) {
        otpValidation = cause;
      } else {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : cause instanceof ApiError && cause.status === 401
              ? "That code is incorrect"
              : "Could not disable two-factor authentication",
        );
      }
    } finally {
      otpBusy = false;
    }
  }
</script>

<PanelShell bind:open={session.menuOpen} label="Folders">
  {#snippet sidebar()}
    <UserSidebar {roots} />
  {/snippet}

  <div
    class="h-[calc(100dvh-3.5rem)] min-h-0 overflow-y-auto overscroll-contain p-4 sm:p-6"
  >
    <div class="mx-auto max-w-3xl space-y-5">
      <div class="border-b pb-3">
        <h2 class="text-lg font-semibold tracking-tight">Settings</h2>
      </div>

      {#if session.user}
        {#if missingEmail || otpOff}
          <div class="space-y-2">
            {#if missingEmail}
              <Banner tone="warning">
                {#snippet icon()}<MailWarning />{/snippet}
                {#if editing}
                  Add an email in Account below so you can recover your account
                  if you lose access.
                {:else}
                  Email not configured. Add one so you can recover your account
                  if you lose access.
                {/if}
                {#snippet action()}
                  {#if !editing}
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onclick={startEditing}
                    >
                      Add email
                    </Button>
                  {/if}
                {/snippet}
              </Banner>
            {/if}
            {#if otpOff}
              <Banner tone="warning">
                {#snippet icon()}<ShieldAlert />{/snippet}
                Two-factor authentication is not enabled.
              </Banner>
            {/if}
          </div>
        {/if}

        <section class="rounded-xl border bg-card p-4 sm:p-5">
          <div
            class="flex flex-wrap items-start justify-between gap-3 {editing
              ? 'mb-4'
              : ''}"
          >
            <h3 class="text-sm font-semibold tracking-tight">Account</h3>
            {#if !editing}
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="shrink-0"
                onclick={startEditing}
              >
                <Pencil size={15} />
                Edit
              </Button>
            {/if}
          </div>

          {#if editing}
            <UserForm
              user={session.user}
              selfService
              save={saveProfile}
              cancel={stopEditing}
            />
          {:else}
            <UserInfo user={session.user} />
          {/if}
        </section>

        <section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
          <h3 class="text-sm font-semibold tracking-tight">Security</h3>

          <div class="space-y-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div class="min-w-0">
                <h4
                  class="flex items-center gap-1.5 text-sm font-medium tracking-tight"
                >
                  {#if session.user.otpEnabled}
                    <ShieldCheck size={15} class="shrink-0 text-success" />
                  {:else}
                    <ShieldAlert size={15} class="shrink-0 text-warning" />
                  {/if}
                  Two-factor authentication
                  <span
                    class="rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium {session
                      .user.otpEnabled
                      ? 'bg-success/15 text-success'
                      : 'bg-warning/20 text-warning'}"
                  >
                    {session.user.otpEnabled ? "Enabled" : "Off"}
                  </span>
                </h4>
                <p class="mt-1 text-xs text-muted-foreground">
                  {#if session.user.otpEnabled}
                    Authenticator codes are required at sign-in.
                  {:else}
                    Use an authenticator app for a second sign-in step.
                  {/if}
                </p>
              </div>

              {#if otpPanel === "idle"}
                <div class="flex shrink-0 gap-1.5">
                  <Button
                    type="button"
                    size="sm"
                    disabled={otpBusy}
                    onclick={startOtpSetup}
                  >
                    {session.user.otpEnabled ? "Change" : "Set up"}
                  </Button>
                  {#if session.user.otpEnabled}
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={otpBusy}
                      onclick={beginDisableOtp}
                    >
                      Disable
                    </Button>
                  {/if}
                </div>
              {/if}
            </div>

            {#if otpPanel === "setup" && qrUrl}
              <div
                class="flex flex-col items-center gap-3 rounded-lg border p-4"
              >
                <p class="text-center text-xs text-muted-foreground">
                  Scan with your authenticator, then enter the 6-digit code.
                </p>
                <div
                  class="flex size-64 items-center justify-center rounded-md border bg-white p-3"
                >
                  <img
                    src={qrUrl}
                    alt="Two-factor authentication QR code"
                    class="size-full object-contain [image-rendering:pixelated]"
                  />
                </div>
                <form
                  class="flex w-full max-w-sm flex-wrap items-end justify-center gap-2"
                  onsubmit={confirmOtpSetup}
                >
                  <div class="min-w-0 flex-1">
                    <Field
                      label="Code"
                      id="otp-setup-code"
                      dense
                      bind:value={otpCode}
                      error={otpValidation?.field("otp_code")}
                      inputmode="numeric"
                      autocomplete="one-time-code"
                      maxlength={6}
                      required
                    />
                  </div>
                  <Button
                    type="submit"
                    size="sm"
                    disabled={otpBusy || otpCode.length !== 6}
                  >
                    Confirm
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={otpBusy}
                    onclick={clearOtpUi}
                  >
                    Cancel
                  </Button>
                </form>
              </div>
            {:else if otpPanel === "disable"}
              <form
                class="space-y-3 rounded-lg border border-destructive/30 bg-destructive/5 p-3"
                onsubmit={confirmDisableOtp}
              >
                <p class="text-xs text-muted-foreground">
                  Enter a current authenticator code to turn off 2FA.
                </p>
                <div class="flex max-w-sm flex-wrap items-end gap-2">
                  <div class="min-w-0 flex-1">
                    <Field
                      label="Code"
                      id="otp-disable-code"
                      dense
                      bind:value={otpCode}
                      error={otpValidation?.field("otp_code")}
                      inputmode="numeric"
                      autocomplete="one-time-code"
                      maxlength={6}
                      required
                    />
                  </div>
                  <Button
                    type="submit"
                    variant="destructive"
                    size="sm"
                    disabled={otpBusy || otpCode.length !== 6}
                  >
                    Disable
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={otpBusy}
                    onclick={clearOtpUi}
                  >
                    Cancel
                  </Button>
                </div>
              </form>
            {/if}
          </div>

          <div class="space-y-3 border-t pt-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div class="min-w-0">
                <h4
                  class="flex items-center gap-1.5 text-sm font-medium tracking-tight"
                >
                  <KeyRound
                    size={15}
                    class="shrink-0 {passkeysEnabled
                      ? 'text-success'
                      : 'text-muted-foreground'}"
                  />
                  Passkeys
                  <span
                    class="rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium {passkeysEnabled
                      ? 'bg-success/15 text-success'
                      : 'bg-muted text-muted-foreground'}"
                  >
                    {passkeysEnabled ? "Enabled" : "None"}
                  </span>
                </h4>
              </div>

              {#if !session.native}
                <Button
                  type="button"
                  size="sm"
                  class="shrink-0"
                  disabled={passkeyBusy || deletingPasskey}
                  onclick={addPasskey}
                >
                  Add passkey
                </Button>
              {/if}
            </div>

            {#if session.native}
              <p class="text-xs text-muted-foreground">
                Passkey registration available only on web.
              </p>
            {/if}

            {#if passkeys.length > 0}
              <ul class="space-y-1.5">
                {#each passkeys as passkey (passkey.id)}
                  <li
                    class="flex items-center gap-2 rounded-md border bg-muted/30 px-2.5 py-1.5 text-xs text-muted-foreground"
                  >
                    <span class="min-w-0 flex-1 truncate">
                      Passkey · {formatPasskeyDate(passkey.createdAt)} · {passkey.algorithm}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      class="shrink-0 text-muted-foreground hover:text-destructive"
                      disabled={passkeyBusy || deletingPasskey}
                      aria-label="Remove passkey"
                      onclick={() => (deletePasskeyId = passkey.id)}
                    >
                      <Trash size={15} />
                    </Button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        </section>

        <ActiveSessions />
      {/if}
    </div>
  </div>
</PanelShell>

<ConfirmDialog
  open={deletePasskeyId !== null}
  title="Remove passkey?"
  description="You will no longer be able to sign in with this passkey."
  confirmLabel="Remove"
  busy={deletingPasskey}
  onCancel={() => (deletePasskeyId = null)}
  onConfirm={() => removePasskey()}
/>
