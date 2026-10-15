<script lang="ts">
  import { onMount } from "svelte";
  import { ValidationError } from "../api";
  import { toastError } from "../toast";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
  import Chevron from "../components/ui/chevron/chevron.svelte";
  import Field from "../components/ui/field/field.svelte";
  import { navigate } from "../router";
  import { session } from "../session.svelte";

  let username = $state("");
  let password = $state("");
  let remember = $state(false);
  let otp = $state("");
  let pendingToken = $state<string | null>(null);
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);
  let checking = $state(true);
  let draft = $state(session.endpoint ?? "");
  let serverExpanded = $state(session.endpoint == null);

  function normalizeEndpoint(url: string): string {
    return url.trim().replace(/\/$/, "");
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

  async function submit(event: SubmitEvent) {
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

      const result = pendingToken
        ? await session.client.loginOtp(pendingToken, otp)
        : await session.client.login(username, password, {
            remember: session.native ? false : remember,
            permanent: session.native,
          });

      if (result.status === "twoFactorRequired") {
        pendingToken = result.token;
        return;
      }

      session.user = result.session.user;

      navigate("/files", { replace: true });
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
</script>

{#if checking}
  <p class="p-8 text-sm text-muted-foreground">Loading…</p>
{:else}
  <main class="flex min-h-screen items-center justify-center p-6">
    <Card class="w-full max-w-md p-6">
      <form class="space-y-4" onsubmit={submit}>
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
                  <span class="mt-0.5 block truncate text-xs text-muted-foreground">
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

        {#if pendingToken}
          <Field
            label="Authenticator code"
            id="otp"
            bind:value={otp}
            error={validation?.field("otp_code")}
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength={6}
          />
        {:else}
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
            <label class="flex items-center gap-2 text-sm">
              <input type="checkbox" bind:checked={remember} />
              Remember this session
            </label>
          {/if}
        {/if}

        <Button type="submit" class="w-full" disabled={busy}>
          {pendingToken ? "Verify" : "Sign in"}
        </Button>
        {#if !pendingToken}
          <Button
            type="button"
            variant="ghost"
            class="w-full"
            onclick={() => navigate("/reset-password")}
          >
            Forgot your password?
          </Button>
        {/if}
      </form>
    </Card>
  </main>
{/if}
