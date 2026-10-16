<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Check, KeyRound, X } from "@lucide/svelte";
  import AuthShell from "../components/AuthShell.svelte";
  import Button from "../components/ui/button/button.svelte";
  import { session } from "../session.svelte";
  import { getPasskey } from "../webauthn";

  type Status = "missing" | "verifying" | "success" | "cancelled" | "error";

  const CLOSE_AFTER_SECONDS = 5;

  let status = $state<Status>("verifying");
  let errorMessage = $state("");
  let token = $state<string | null>(null);
  let busy = $state(false);
  let closeIn = $state(CLOSE_AFTER_SECONDS);
  let closeTimer: ReturnType<typeof setInterval> | null = null;

  const closeProgress = $derived(
    ((CLOSE_AFTER_SECONDS - closeIn) / CLOSE_AFTER_SECONDS) * 100,
  );

  function clearCloseTimer() {
    if (closeTimer !== null) {
      clearInterval(closeTimer);
      closeTimer = null;
    }
  }

  function startCloseCountdown() {
    clearCloseTimer();
    closeIn = CLOSE_AFTER_SECONDS;

    closeTimer = setInterval(() => {
      closeIn -= 1;

      if (closeIn <= 0) {
        clearCloseTimer();
        window.close();
      }
    }, 1000);
  }

  function closeWindow() {
    clearCloseTimer();
    window.close();
  }

  function isWebAuthnCancelled(cause: unknown): boolean {
    return (
      (cause instanceof DOMException || cause instanceof Error) &&
      (cause.name === "NotAllowedError" || cause.name === "AbortError")
    );
  }

  async function runFlow() {
    if (!token) {
      status = "missing";
      return;
    }

    busy = true;
    status = "verifying";
    errorMessage = "";
    clearCloseTimer();

    try {
      const challenge = await session.client.startPasskeyLogin({ token });
      const credential = await getPasskey(challenge.challenge);
      await session.client.confirmNativePasskeyLogin(
        challenge.token,
        credential,
      );
      status = "success";
      startCloseCountdown();
    } catch (cause) {
      console.error(cause);

      if (isWebAuthnCancelled(cause)) {
        status = "cancelled";
        return;
      }

      status = "error";
      errorMessage =
        cause instanceof TypeError
          ? "The server could not be reached."
          : "Passkey verification failed. Try again, or return to the app.";
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    const value = params.get("token");

    if (!value) {
      status = "missing";
      return;
    }

    token = value;
    void runFlow();
  });

  onDestroy(() => {
    clearCloseTimer();
  });
</script>

<AuthShell>
  <section class="auth-panel bridge-panel" aria-live="polite">
    <p class="bridge-brand">Nimbus</p>

    <div class="bridge-copy">
      <p class="bridge-eyebrow">Desktop sign-in</p>
      <h1 class="bridge-title">
        {#if status === "success"}
          You're verified
        {:else if status === "cancelled"}
          Verification cancelled
        {:else if status === "error"}
          Verification failed
        {:else if status === "missing"}
          Invalid link
        {:else}
          Confirm your passkey
        {/if}
      </h1>
      <p class="bridge-lede">
        {#if status === "missing"}
          Open this page again from the Nimbus app to continue.
        {:else if status === "success"}
          Return to the app to finish signing in. This tab closes automatically.
        {:else if status === "cancelled"}
          The authenticator prompt was dismissed. Retry here, or go back to the
          app.
        {:else if status === "error"}
          {errorMessage}
        {:else}
          Choose a passkey for this site. This confirms sign-in for the desktop
          app.
        {/if}
      </p>
    </div>

    {#if status === "verifying"}
      <div class="bridge-status">
        <div class="bridge-icon bridge-icon-pulse">
          <KeyRound size={20} strokeWidth={1.75} />
        </div>
        <div>
          <p class="bridge-status-title">Waiting for authenticator</p>
          <p class="bridge-status-meta">Complete the browser prompt to continue</p>
        </div>
      </div>
    {:else if status === "success"}
      <div class="bridge-status bridge-status-success">
        <div class="bridge-icon bridge-icon-success">
          <Check size={20} strokeWidth={2} />
        </div>
        <div class="min-w-0 flex-1">
          <p class="bridge-status-title">Passkey accepted</p>
          <p class="bridge-status-meta">Closing in {closeIn}s</p>
          <div class="bridge-progress" aria-hidden="true">
            <div
              class="bridge-progress-bar"
              style="width: {closeProgress}%"
            ></div>
          </div>
        </div>
      </div>

      <Button type="button" variant="outline" class="w-full" onclick={closeWindow}>
        Close now
      </Button>
    {:else if status === "cancelled" || status === "error"}
      <div class="bridge-status">
        <div class="bridge-icon">
          <X size={20} strokeWidth={1.75} />
        </div>
        <div>
          <p class="bridge-status-title">
            {status === "cancelled" ? "No credential returned" : "Could not verify"}
          </p>
          <p class="bridge-status-meta">
            You can retry without leaving this page
          </p>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <Button
          type="button"
          class="w-full"
          disabled={busy}
          onclick={() => void runFlow()}
        >
          Try again
        </Button>
        <Button
          type="button"
          variant="ghost"
          class="w-full"
          onclick={closeWindow}
        >
          Close
        </Button>
      </div>
    {:else if status === "missing"}
      <Button
        type="button"
        variant="outline"
        class="w-full"
        onclick={closeWindow}
      >
        Close
      </Button>
    {/if}
  </section>
</AuthShell>

<style>
  .bridge-panel {
    display: flex;
    width: 100%;
    max-width: 26rem;
    flex-direction: column;
    gap: 1.75rem;
    border: 1px solid var(--border);
    border-radius: calc(var(--radius) + 0.15rem);
    padding: 2rem 1.75rem 1.75rem;
  }

  .bridge-brand {
    margin: 0;
    font-size: 1.75rem;
    font-weight: 650;
    letter-spacing: -0.035em;
    line-height: 1.1;
  }

  .bridge-copy {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .bridge-eyebrow {
    margin: 0;
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted-foreground);
  }

  .bridge-title {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.25;
  }

  .bridge-lede {
    margin: 0.15rem 0 0;
    max-width: 34ch;
    font-size: 0.9rem;
    line-height: 1.5;
    color: var(--muted-foreground);
  }

  .bridge-status {
    display: flex;
    align-items: flex-start;
    gap: 0.875rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: color-mix(in oklab, var(--muted) 55%, transparent);
    padding: 1rem 1.05rem;
  }

  .bridge-status-success {
    border-color: color-mix(in oklab, var(--success) 35%, var(--border));
    background: color-mix(in oklab, var(--success) 10%, var(--card));
  }

  .bridge-icon {
    display: flex;
    width: 2.5rem;
    height: 2.5rem;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    border-radius: 0.65rem;
    border: 1px solid var(--border);
    background: var(--background);
    color: var(--foreground);
  }

  .bridge-icon-success {
    border-color: color-mix(in oklab, var(--success) 40%, var(--border));
    color: var(--success);
  }

  .bridge-icon-pulse {
    animation: bridge-pulse 1.6s ease-in-out infinite;
  }

  .bridge-status-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
  }

  .bridge-status-meta {
    margin: 0.2rem 0 0;
    font-size: 0.75rem;
    color: var(--muted-foreground);
  }

  .bridge-progress {
    margin-top: 0.7rem;
    height: 3px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 10%, transparent);
  }

  .bridge-progress-bar {
    height: 100%;
    border-radius: inherit;
    background: var(--primary);
    transition: width 1s linear;
  }

  @keyframes bridge-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 0 color-mix(in oklab, var(--primary) 0%, transparent);
    }
    50% {
      box-shadow: 0 0 0 6px color-mix(in oklab, var(--primary) 16%, transparent);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bridge-icon-pulse,
    .bridge-progress-bar {
      animation: none;
      transition: none;
    }
  }
</style>
