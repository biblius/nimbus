<script lang="ts">
  import { onMount } from "svelte";
  import { ArrowLeft, KeyRound } from "@lucide/svelte";
  import { ApiError, ValidationError } from "../api";
  import { toastError } from "../toast";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
  import Checkbox from "../components/ui/checkbox/checkbox.svelte";
  import Chevron from "../components/ui/chevron/chevron.svelte";
  import Field from "../components/ui/field/field.svelte";
  import { navigate } from "../router";
  import { session } from "../session.svelte";
  import { getPasskey } from "../webauthn";

  type Step = "credentials" | "otp" | "passkey";

  let username = $state("");
  let password = $state("");
  let remember = $state(false);
  let otp = $state("");
  let pendingToken = $state<string | null>(null);
  let step = $state<Step>("credentials");
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);
  let checking = $state(true);
  let draft = $state(session.endpoint ?? "");
  let serverExpanded = $state(session.endpoint == null);
  let passkeyAbort: AbortController | null = null;

  onMount(async () => {
    if (session.native && !session.endpoint) {
      checking = false;
      serverExpanded = true;
      return;
    }

    try {
      session.user = await session.client.me();
      navigate("/", { replace: true });
    } catch {
      checking = false;
    }
  });

  function normalizeEndpoint(url: string): string {
    return url.trim().replace(/\/$/, "");
  }

  function isWebAuthnCancelled(cause: unknown): boolean {
    return (
      (cause instanceof DOMException || cause instanceof Error) &&
      (cause.name === "NotAllowedError" || cause.name === "AbortError")
    );
  }

  function cancelPasskeyWait() {
    passkeyAbort?.abort();
    passkeyAbort = null;
  }

  function leavePasskeyStep() {
    cancelPasskeyWait();
    busy = false;
    step = "credentials";
  }

  function setSession(user: NonNullable<typeof session.user>) {
    session.user = user;
    navigate("/files", { replace: true });
  }

  // Native only
  async function ensureNativeEndpoint(): Promise<boolean> {
    const normalized = normalizeEndpoint(draft);

    if (normalized !== (session.endpoint ?? "")) {
      // Native sessions always have this
      await session.configureEndpoint!(draft);
      draft = session.endpoint ?? draft;
    }

    if (!session.endpoint) {
      toastError("Configure a server URL first");
      serverExpanded = true;
      return false;
    }

    return true;
  }

  async function beginPasskeyLogin() {
    cancelPasskeyWait();

    validation = undefined;
    busy = true;
    step = "passkey";

    try {
      // Native clients handle WebAuthn in the system browser.
      if (session.native) {
        if (!(await ensureNativeEndpoint())) {
          step = "credentials";
          return;
        }

        await startNativePasskeyLogin();
        return;
      }

      const challenge = await session.client.startPasskeyLogin({
        remember,
      });

      const credential = await getPasskey(challenge.challenge);

      const result = await session.client.finishPasskeyLogin(
        challenge.token,
        credential,
      );

      if (result.status !== "session") {
        toastError("Passkey sign-in failed");
        step = "credentials";
        return;
      }

      const user = await session.client.me();
      setSession(user);
    } catch (cause) {
      console.error(cause);
      if (!isWebAuthnCancelled(cause)) {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Passkey sign-in failed",
        );
      }

      step = "credentials";
    } finally {
      busy = false;
    }
  }

  // Native only: open the browser bridge and poll for the session.
  async function startNativePasskeyLogin() {
    const controller = new AbortController();

    passkeyAbort = controller;

    try {
      if (!session.openExternalUrl) {
        throw new Error("External browser is not available");
      }

      const challenge = await session.client.startPasskeyLogin({
        permanent: true,
      });

      await session.openExternalUrl(
        `${challenge.frontendUrl}/login/passkey?token=${encodeURIComponent(challenge.token)}`,
      );

      while (!controller.signal.aborted) {
        try {
          const result = await session.client.finishNativeLogin(
            challenge.token,
          );

          if (result.status !== "session") {
            toastError("Passkey sign-in failed");
            step = "credentials";
            return;
          }

          const user = await session.client.me();
          setSession(user);
          return;
        } catch (cause) {
          // 404 on `finishNativeLogin` means the token is not ready yet.
          if (cause instanceof ApiError && cause.status === 404) {
            await new Promise((resolve) => setTimeout(resolve, 1500));
            continue;
          }

          throw cause;
        }
      }
    } catch (cause) {
      if (controller.signal.aborted) {
        return;
      }

      console.error(cause);

      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Passkey sign-in failed",
      );

      step = "credentials";
    } finally {
      if (passkeyAbort === controller) {
        passkeyAbort = null;
      }

      if (!controller.signal.aborted) {
        busy = false;
      }
    }
  }

  // Native only
  async function saveServer() {
    if (!session.configureEndpoint) {
      return;
    }

    const normalized = normalizeEndpoint(draft);

    if (!normalized) {
      toastError("Enter a server URL");
      return;
    }

    busy = true;

    try {
      await session.configureEndpoint(normalized);
      draft = session.endpoint ?? normalized;
      serverExpanded = false;

      try {
        session.user = await session.client.me();
        navigate("/", { replace: true });
      } catch {
        // Stay on login — credentials still required.
      }
    } catch {
      toastError("Could not save server URL");
    } finally {
      busy = false;
    }
  }

  async function submitCredentials(event: SubmitEvent) {
    event.preventDefault();

    validation = undefined;
    busy = true;

    try {
      if (session.native && !(await ensureNativeEndpoint())) {
        return;
      }

      const result = await session.client.login(username, password, {
        remember: session.native ? false : remember,
        permanent: session.native,
      });

      if (result.status === "twoFactorRequired") {
        pendingToken = result.token;
        otp = "";
        validation = undefined;
        step = "otp";
        return;
      }

      if (result.status !== "session") {
        toastError("Sign-in failed");
        return;
      }

      const user = await session.client.me();
      setSession(user);
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Sign-in failed",
      );
    } finally {
      busy = false;
    }
  }

  async function submitOtp(event: SubmitEvent) {
    event.preventDefault();

    if (!pendingToken) {
      return;
    }

    validation = undefined;
    busy = true;

    try {
      const result = await session.client.loginOtp(pendingToken, otp);

      if (result.status !== "session") {
        toastError("Sign-in failed");
        return;
      }

      const user = await session.client.me();
      setSession(user);
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Sign-in failed",
      );
    } finally {
      busy = false;
    }
  }

  function backToCredentials() {
    cancelPasskeyWait();
    pendingToken = null;
    otp = "";
    step = "credentials";
    validation = undefined;
  }
</script>

<main class="bridge">
  <div class="bridge-glow" aria-hidden="true"></div>

  {#if checking}
    <p class="relative z-10 p-8 text-sm text-muted-foreground">Loading…</p>
  {:else}
    <Card class="relative z-10 w-full max-w-md p-6">
      {#if step === "credentials"}
        <form class="space-y-4" onsubmit={submitCredentials}>
          <div>
            <h1 class="text-2xl font-semibold tracking-tight">Nimbus</h1>
          </div>

          {#if session.native}
            <div class="space-y-2">
              <button
                type="button"
                class="flex w-full items-center gap-2 rounded-md px-1 py-1.5 text-left transition-colors hover:bg-accent/50"
                aria-expanded={serverExpanded}
                onclick={() => (serverExpanded = !serverExpanded)}
              >
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-medium">Server</span>
                  {#if !serverExpanded && session.endpoint}
                    <span
                      class="mt-0.5 block truncate text-xs text-muted-foreground"
                    >
                      {session.endpoint}
                    </span>
                  {/if}
                </span>
                <Chevron expanded={serverExpanded} />
              </button>

              {#if serverExpanded}
                <div class="space-y-3">
                  <Field
                    label="Server URL"
                    id="server-url"
                    type="url"
                    placeholder="https://nimbus.example.com"
                    bind:value={draft}
                    autocomplete="url"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    class="w-full"
                    disabled={busy}
                    onclick={saveServer}
                  >
                    Save
                  </Button>
                </div>
              {/if}
            </div>
          {/if}

          <Field
            label="Username"
            id="username"
            bind:value={username}
            error={validation?.field("username")}
            autocomplete="username"
          />
          <Field
            label="Password"
            id="password"
            type="password"
            bind:value={password}
            error={validation?.field("password")}
            autocomplete="current-password"
          />

          {#if !session.native}
            <label class="flex cursor-pointer items-center gap-2 text-sm">
              <Checkbox bind:checked={remember} />
              Remember this session
            </label>
          {/if}

          <Button type="submit" class="w-full" disabled={busy}>Sign in</Button>

          <div class="relative py-1">
            <div class="absolute inset-0 flex items-center" aria-hidden="true">
              <div class="w-full border-t border-border"></div>
            </div>
            <div class="relative flex justify-center text-xs">
              <span class="bg-card px-2 text-muted-foreground">or</span>
            </div>
          </div>

          <Button
            type="button"
            variant="outline"
            class="w-full"
            disabled={busy}
            onclick={() => void beginPasskeyLogin()}
          >
            <KeyRound size={16} class="mr-2" />
            Sign in with passkey
          </Button>

          <Button
            type="button"
            variant="ghost"
            class="w-full"
            onclick={() => navigate("/reset-password")}
          >
            Forgot your password?
          </Button>
        </form>
      {:else if step === "passkey"}
        <div class="space-y-6">
          <div class="space-y-1">
            <p
              class="text-xs font-medium tracking-wide text-muted-foreground uppercase"
            >
              Passkey
            </p>
            <h1 class="text-2xl font-semibold tracking-tight">Sign in</h1>
            <p class="text-sm text-muted-foreground">
              {#if session.native}
                Complete the passkey in the browser window that just opened.
              {:else}
                Confirm with your device or password manager to continue.
              {/if}
            </p>
          </div>

          <div
            class="flex items-center gap-3 rounded-xl border border-dashed bg-muted/30 px-4 py-5"
          >
            <div
              class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-background shadow-sm ring-1 ring-border"
            >
              <KeyRound size={18} class="text-foreground/80" />
            </div>
            <div class="min-w-0">
              <p class="text-sm font-medium">Waiting for your passkey…</p>
              <p class="mt-0.5 text-xs text-muted-foreground">
                {#if session.native}
                  Finish verification in the browser, or cancel to go back.
                {:else}
                  Complete the browser prompt, or cancel to go back.
                {/if}
              </p>
            </div>
          </div>

          <Button
            type="button"
            variant="outline"
            class="w-full"
            onclick={leavePasskeyStep}
          >
            Cancel
          </Button>
        </div>
      {:else}
        <form class="space-y-4" onsubmit={submitOtp}>
          <div class="space-y-1">
            <button
              type="button"
              class="mb-2 inline-flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
              onclick={backToCredentials}
            >
              <ArrowLeft size={14} />
              Back to sign in
            </button>
            <p
              class="text-xs font-medium tracking-wide text-muted-foreground uppercase"
            >
              Verification
            </p>
            <h1 class="text-2xl font-semibold tracking-tight">
              Authenticator code
            </h1>
            <p class="text-sm text-muted-foreground">
              Enter the code from your authenticator app.
            </p>
          </div>

          <Field
            label="Code"
            id="otp"
            bind:value={otp}
            error={validation?.field("otp_code")}
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength={6}
          />

          <Button type="submit" class="w-full" disabled={busy}>Verify</Button>
        </form>
      {/if}
    </Card>
  {/if}
</main>

<style>
  .bridge {
    position: relative;
    display: flex;
    min-height: 100dvh;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    padding: 1.5rem;
  }

  .bridge-glow {
    pointer-events: none;
    position: absolute;
    inset: 0;
    background:
      radial-gradient(
        ellipse 70% 45% at 50% 0%,
        color-mix(in oklab, var(--primary) 22%, transparent),
        transparent 70%
      ),
      radial-gradient(
        ellipse 50% 40% at 85% 90%,
        color-mix(in oklab, var(--primary) 8%, transparent),
        transparent 60%
      ),
      var(--background);
  }
</style>
