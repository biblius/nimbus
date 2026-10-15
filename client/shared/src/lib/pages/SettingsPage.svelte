<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Pencil, ShieldAlert, ShieldCheck } from "@lucide/svelte";
  import { ApiError, ValidationError } from "../api";
  import ActiveSessions from "../components/ActiveSessions.svelte";
  import PanelShell from "../components/PanelShell.svelte";
  import Button from "../components/ui/button/button.svelte";
  import Field from "../components/ui/field/field.svelte";
  import UserForm from "../components/user/UserForm.svelte";
  import UserInfo from "../components/user/UserInfo.svelte";
  import UserSidebar from "../components/UserSidebar.svelte";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { Root, UserData } from "../types";

  type OtpPanel = "idle" | "setup" | "disable";

  let roots = $state<Root[]>([]);
  let editing = $state(false);
  let otpPanel = $state<OtpPanel>("idle");
  let otpBusy = $state(false);
  let otpCode = $state("");
  let otpToken = $state<string | null>(null);
  let qrUrl = $state<string | null>(null);
  let otpValidation = $state<ValidationError | undefined>();

  onMount(async () => {
    try {
      roots = await session.client.roots();
    } catch {
      toastError("Could not load files");
    }
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

  function stopEditing() {
    clearOtpUi();
    editing = false;
  }

  async function saveProfile(data: UserData) {
    const updated = await session.client.updateMe({
      username: data.username?.trim(),
      email: data.email || undefined,
      password: data.password || undefined,
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

<PanelShell bind:open={session.menuOpen} label="Roots">
  {#snippet sidebar()}
    <UserSidebar {roots} />
  {/snippet}

  <div class="p-4 sm:p-6">
    <div class="mx-auto max-w-3xl space-y-5">
      <div class="space-y-0.5 border-b pb-3">
        <h2 class="text-lg font-semibold tracking-tight">Account</h2>
        <p class="text-sm text-muted-foreground">
          Profile, security, and signed-in sessions.
        </p>
      </div>

      {#if session.user}
        <section class="rounded-xl border bg-card p-4 sm:p-5">
          {#if editing}
            <div class="space-y-5">
              <UserForm
                user={session.user}
                selfService
                title="Edit account"
                description="Blank password keeps the current one."
                save={saveProfile}
                cancel={stopEditing}
              />

              <div
                class="space-y-3 rounded-lg border p-3 {session.user.otpEnabled
                  ? 'border-success/25 bg-success/5'
                  : 'border-warning/35 bg-warning/8'}"
              >
                <div class="flex flex-wrap items-start justify-between gap-2">
                  <div class="min-w-0">
                    <h3
                      class="flex items-center gap-1.5 text-sm font-semibold tracking-tight"
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
                    </h3>
                    <p
                      class="mt-1 text-xs {session.user.otpEnabled
                        ? 'text-muted-foreground'
                        : 'text-warning/90'}"
                    >
                      {#if session.user.otpEnabled}
                        Authenticator codes are required at sign-in.
                      {:else}
                        Your account is only protected by a password. Enable
                        2FA.
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
                    class="flex flex-col items-center gap-3 rounded-lg border bg-muted/20 p-4"
                  >
                    <p class="text-center text-xs text-muted-foreground">
                      Scan with your authenticator, then enter the 6-digit code.
                    </p>
                    <div
                      class="flex size-44 items-center justify-center rounded-md border bg-white p-3"
                    >
                      <img
                        src={qrUrl}
                        alt="Two-factor authentication QR code"
                        class="size-full object-contain"
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
            </div>
          {:else}
            <div
              class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="min-w-0 flex-1">
                <UserInfo user={session.user} />
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="shrink-0"
                onclick={() => (editing = true)}
              >
                <Pencil size={15} />
                Edit
              </Button>
            </div>
          {/if}
        </section>

        <ActiveSessions />
      {/if}
    </div>
  </div>
</PanelShell>
