<script lang="ts">
  import { ShieldAlert } from "@lucide/svelte";
  import { ValidationError } from "../api";
  import { toastError } from "../toast";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
  import Field from "../components/ui/field/field.svelte";
  import { navigate } from "../router";
  import { session } from "../session.svelte";

  let email = $state("");
  let sent = $state(false);
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();

    validation = undefined;

    busy = true;

    try {
      await session.client.requestPasswordReset(email);
      sent = true;
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not request a password reset",
      );
    } finally {
      busy = false;
    }
  }
</script>

<main class="flex min-h-screen items-center justify-center p-6">
  <Card class="w-full max-w-md p-6">
    <div class="space-y-4">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">
          Reset your password
        </h1>
        <p class="text-sm text-muted-foreground">
          Enter your account email and we’ll send you a reset link.
        </p>
      </div>

      <div
        class="flex items-start gap-2 rounded-lg border border-warning/35 bg-warning/8 p-3"
      >
        <ShieldAlert size={15} class="mt-0.5 shrink-0 text-warning" />
        <div class="min-w-0 text-xs text-warning/90">
          <p class="font-semibold tracking-tight text-warning">
            Two-factor authentication will be cleared
          </p>
          <p class="mt-1">
            Completing a reset removes authenticator codes and passkeys, and
            signs out all sessions. You will need to set up 2FA again after
            signing in with your new password.
          </p>
        </div>
      </div>

      {#if sent}
        <p class="text-sm">
          If an account exists for that email, a reset link has been sent.
        </p>
      {:else}
        <form class="space-y-4" onsubmit={submit}>
          <Field
            label="Email"
            id="reset-email"
            type="email"
            bind:value={email}
            error={validation?.field("email")}
            autocomplete="email"
            required
          />

          <Button type="submit" class="w-full" disabled={busy}>
            Send reset link
          </Button>
        </form>
      {/if}

      <Button
        type="button"
        variant="ghost"
        class="w-full"
        onclick={() => navigate("/login")}
      >
        Back to sign in
      </Button>
    </div>
  </Card>
</main>
