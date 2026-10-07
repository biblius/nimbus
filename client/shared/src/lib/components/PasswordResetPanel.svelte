<script lang="ts">
  import { ApiError, type NimbusClient } from "../api";
  import Button from "./ui/button/button.svelte";
  import Card from "./ui/card/card.svelte";
  import Input from "./ui/input/input.svelte";
  import Label from "./ui/label/label.svelte";

  let {
    client,
    token,
    onBack,
  }: {
    client: NimbusClient;
    token?: string;
    onBack: () => void;
  } = $props();

  let email = $state("");
  let password = $state("");
  let confirmation = $state("");
  let error = $state("");
  let complete = $state(false);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = "";

    if (token && password !== confirmation) {
      error = "Passwords do not match";
      return;
    }

    busy = true;
    try {
      if (token) {
        await client.resetPassword(token, password);
      } else {
        await client.requestPasswordReset(email);
      }
      complete = true;
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? token
            ? "This reset link is invalid or expired"
            : "Could not request a password reset"
          : "The server could not be reached";
    } finally {
      busy = false;
    }
  }
</script>

<Card class="w-full max-w-md p-6">
  <div class="space-y-4">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">
        {token ? "Choose a new password" : "Reset your password"}
      </h1>
      <p class="text-sm text-muted-foreground">
        {token
          ? "Enter a new password for your Nimbus account."
          : "Enter your account email and we’ll send you a reset link."}
      </p>
    </div>

    {#if complete}
      <p class="text-sm">
        {token
          ? "Your password has been changed. You can sign in now."
          : "If an account exists for that email, a reset link has been sent."}
      </p>
      <Button type="button" class="w-full" onclick={onBack}
        >Back to sign in</Button
      >
    {:else}
      <form class="space-y-4" onsubmit={submit}>
        {#if token}
          <div class="space-y-2">
            <Label for="reset-password">New password</Label>
            <Input
              id="reset-password"
              type="password"
              bind:value={password}
              autocomplete="new-password"
              required
            />
          </div>
          <div class="space-y-2">
            <Label for="reset-confirmation">Confirm new password</Label>
            <Input
              id="reset-confirmation"
              type="password"
              bind:value={confirmation}
              autocomplete="new-password"
              required
            />
          </div>
        {:else}
          <div class="space-y-2">
            <Label for="reset-email">Email</Label>
            <Input
              id="reset-email"
              type="email"
              bind:value={email}
              autocomplete="email"
              required
            />
          </div>
        {/if}

        {#if error}
          <p class="text-sm text-destructive">{error}</p>
        {/if}

        <Button type="submit" class="w-full" disabled={busy}>
          {token ? "Change password" : "Send reset link"}
        </Button>
        <Button type="button" variant="ghost" class="w-full" onclick={onBack}>
          Back to sign in
        </Button>
      </form>
    {/if}
  </div>
</Card>
