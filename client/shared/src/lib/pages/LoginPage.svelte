<script lang="ts">
  import { ValidationError } from "../api";
  import { toastError } from "../toast";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
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

  async function submit(event: SubmitEvent) {
    event.preventDefault();

    validation = undefined;

    busy = true;

    try {
      const result = pendingToken
        ? await session.client.loginOtp(pendingToken, otp)
        : await session.client.login(username, password, remember);

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

<main class="flex min-h-screen items-center justify-center p-6">
  <Card class="w-full max-w-md p-6">
    <form class="space-y-4" onsubmit={submit}>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Nimbus</h1>
      </div>

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

        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={remember} />
          Remember this session
        </label>
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
