<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowLeft,
    ChevronRight,
    KeyRound,
    ShieldCheck,
  } from "@lucide/svelte";
  import { ValidationError } from "../api";
  import { toastError } from "../toast";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
  import Checkbox from "../components/ui/checkbox/checkbox.svelte";
  import Chevron from "../components/ui/chevron/chevron.svelte";
  import Field from "../components/ui/field/field.svelte";
  import { navigate } from "../router";
  import { session } from "../session.svelte";
  import type { TFAType } from "../types";
  import { getPasskey } from "../webauthn";

  type Step = "credentials" | "methods" | "otp" | "passkey";

  let username = $state("");
  let password = $state("");
  let remember = $state(false);
  let otp = $state("");
  let pendingToken = $state<string | null>(null);
  let available = $state<TFAType[]>([]);
  let step = $state<Step>("credentials");
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);
  let checking = $state(true);
  let draft = $state(session.endpoint ?? "");
  let serverExpanded = $state(session.endpoint == null);

  function normalizeEndpoint(url: string): string {
    return url.trim().replace(/\/$/, "");
  }

  function isWebAuthnCancelled(cause: unknown): boolean {
    return (
      (cause instanceof DOMException || cause instanceof Error) &&
      (cause.name === "NotAllowedError" || cause.name === "AbortError")
    );
  }

  function setSession(user: NonNullable<typeof session.user>) {
    session.user = user;
    navigate("/files", { replace: true });
  }

  async function beginPasskeyLogin(token: string) {
    busy = true;

    step = "passkey";

    try {
      const challenge = await session.client.startPasskeyLogin(token);

      const credential = await getPasskey(challenge.challenge);

      const result = await session.client.finishPasskeyLogin(
        challenge.token,
        credential,
      );

      if (result.status !== "session") {
        toastError("Passkey sign-in failed");
        step = "methods";
        return;
      }

      setSession(result.session.user);
    } catch (cause) {
      console.error(cause);
      if (!isWebAuthnCancelled(cause)) {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Passkey sign-in failed",
        );
      }

      step = "methods";
    } finally {
      busy = false;
    }
  }

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
      if (session.native && session.configureEndpoint) {
        const normalized = normalizeEndpoint(draft);

        if (normalized !== (session.endpoint ?? "")) {
          await session.configureEndpoint(draft);
          draft = session.endpoint ?? draft;
        }

        if (!session.endpoint) {
          toastError("Configure a server URL first");
          serverExpanded = true;
          return;
        }
      }

      const result = await session.client.login(username, password, {
        remember: session.native ? false : remember,
        permanent: session.native,
      });

      if (result.status === "twoFactorRequired") {
        pendingToken = result.token;
        available = result.available;
        otp = "";
        validation = undefined;

        if (result.available.includes("passkey")) {
          await beginPasskeyLogin(result.token);
        } else {
          step = "methods";
        }
        return;
      }

      setSession(result.session.user);
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
      if (step !== "passkey") {
        busy = false;
      }
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

      setSession(result.session.user);
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
    pendingToken = null;
    available = [];
    otp = "";
    step = "credentials";
    validation = undefined;
  }
</script>

{#if checking}
  <p class="p-8 text-sm text-muted-foreground">Loading…</p>
{:else}
  <main class="flex min-h-screen items-center justify-center p-6">
    <Card class="w-full max-w-md p-6">
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
              Verification
            </p>
            <h1 class="text-2xl font-semibold tracking-tight">Passkey</h1>
            <p class="text-sm text-muted-foreground">
              Confirm with your device or password manager to continue.
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
                Complete the browser prompt, or choose another method.
              </p>
            </div>
          </div>

          <Button
            type="button"
            variant="outline"
            class="w-full"
            disabled={busy}
            onclick={() => (step = "methods")}
          >
            Use another method
          </Button>
        </div>
      {:else if step === "methods"}
        <div class="space-y-6">
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
              Choose a method
            </h1>
            <p class="text-sm text-muted-foreground">
              Finish signing in with one of the options below.
            </p>
          </div>

          <div class="space-y-2">
            {#if available.includes("passkey")}
              <button
                type="button"
                class="group flex w-full items-center gap-3 rounded-xl border bg-background p-4 text-left transition-colors hover:border-foreground/15 hover:bg-accent/40 disabled:opacity-60"
                disabled={busy}
                onclick={() => {
                  if (pendingToken) {
                    void beginPasskeyLogin(pendingToken);
                  }
                }}
              >
                <div
                  class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-muted ring-1 ring-border"
                >
                  <KeyRound size={18} class="text-foreground/80" />
                </div>
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-semibold tracking-tight"
                    >Passkey</span
                  >
                  <span class="mt-0.5 block text-xs text-muted-foreground">
                    Use Face ID, Touch ID, a security key, or your password
                    manager
                  </span>
                </span>
                <ChevronRight
                  size={16}
                  class="shrink-0 text-muted-foreground transition-colors group-hover:text-foreground"
                />
              </button>
            {/if}

            {#if available.includes("otp")}
              <button
                type="button"
                class="group flex w-full items-center gap-3 rounded-xl border bg-background p-4 text-left transition-colors hover:border-foreground/15 hover:bg-accent/40 disabled:opacity-60"
                disabled={busy}
                onclick={() => {
                  otp = "";
                  validation = undefined;
                  step = "otp";
                }}
              >
                <div
                  class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-muted ring-1 ring-border"
                >
                  <ShieldCheck size={18} class="text-foreground/80" />
                </div>
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-semibold tracking-tight"
                    >Authenticator code</span
                  >
                  <span class="mt-0.5 block text-xs text-muted-foreground">
                    Enter the 6-digit code from your authenticator app
                  </span>
                </span>
                <ChevronRight
                  size={16}
                  class="shrink-0 text-muted-foreground transition-colors group-hover:text-foreground"
                />
              </button>
            {/if}
          </div>
        </div>
      {:else}
        <form class="space-y-4" onsubmit={submitOtp}>
          <div class="space-y-1">
            <button
              type="button"
              class="mb-2 inline-flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
              onclick={() => (step = "methods")}
            >
              <ArrowLeft size={14} />
              Other methods
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
  </main>
{/if}
