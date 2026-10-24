<script lang="ts">
  import { ShieldAlert } from "@lucide/svelte";
  import { ValidationError } from "../api-errors";
  import AuthShell from "../components/AuthShell.svelte";
  import Button from "../components/ui/button/button.svelte";
  import Card from "../components/ui/card/card.svelte";
  import Field from "../components/ui/field/field.svelte";
  import { navigate, route } from "../router";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";

  let password = $state("");
  let confirmation = $state("");
  let mismatch = $state("");
  let validation = $state<ValidationError | undefined>();
  let changed = $state(false);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();

    mismatch = "";

    validation = undefined;

    if (password !== confirmation) {
      mismatch = "Passwords do not match";
      return;
    }

    busy = true;

    try {
      await session.client.resetPassword(route.params.token, password);

      changed = true;
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "This reset link is invalid or expired",
      );
    } finally {
      busy = false;
    }
  }
</script>

<AuthShell>
  <Card class="auth-panel w-full max-w-md p-6">
    <div class="space-y-4">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">
          Choose a new password
        </h1>
        <p class="text-sm text-muted-foreground">
          Enter a new password for your Nimbus account.
        </p>
      </div>

      {#if !changed}
        <div
          class="flex items-start gap-2 rounded-lg border border-warning/35 bg-warning/8 p-3"
        >
          <ShieldAlert size={15} class="mt-0.5 shrink-0 text-warning" />
          <div class="min-w-0 text-xs text-warning/90">
            <p class="font-semibold tracking-tight text-warning">
              Two-factor authentication will be cleared
            </p>
            <p class="mt-1">
              Changing your password removes authenticator codes and passkeys,
              and signs out all sessions. Set up 2FA again after you sign in.
            </p>
          </div>
        </div>
      {/if}

      {#if changed}
        <p class="text-sm">
          Your password has been changed. You can sign in now.
        </p>
      {:else}
        <form class="space-y-4" onsubmit={submit}>
          <Field
            label="New password"
            id="reset-password"
            type="password"
            bind:value={password}
            error={validation?.field("new_password")}
            autocomplete="new-password"
            required
          />
          <Field
            label="Confirm new password"
            id="reset-confirmation"
            type="password"
            bind:value={confirmation}
            class={mismatch
              ? "border-destructive focus-visible:ring-destructive"
              : ""}
            autocomplete="new-password"
            required
            aria-invalid={mismatch ? true : undefined}
          />
          {#if mismatch}
            <p class="text-sm text-destructive">{mismatch}</p>
          {/if}

          <Button type="submit" class="w-full" disabled={busy}>
            Change password
          </Button>
        </form>
      {/if}

      <Button
        type="button"
        variant={changed ? "default" : "ghost"}
        class="w-full"
        onclick={() => navigate("/login")}
      >
        Back to sign in
      </Button>
    </div>
  </Card>
</AuthShell>
