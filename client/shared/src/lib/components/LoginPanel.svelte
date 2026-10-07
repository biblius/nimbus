<script lang="ts">
  import { ApiError, type NimbusClient } from "../api";
  import type { UserView } from "../types";
  import Button from "./ui/button/button.svelte";
  import Card from "./ui/card/card.svelte";
  import Input from "./ui/input/input.svelte";
  import Label from "./ui/label/label.svelte";

  let {
    client,
    onSuccess,
  }: {
    client: NimbusClient;
    onSuccess: (user: UserView, firstLogin: boolean) => void;
  } = $props();

  let username = $state("");
  let password = $state("");
  let remember = $state(false);
  let otp = $state("");
  let pendingToken = $state<string | null>(null);
  let error = $state("");
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    busy = true;
    try {
      const result = pendingToken
        ? await client.loginOtp(pendingToken, otp)
        : await client.login(username, password, remember);
      if (result.status === "twoFactorRequired") {
        pendingToken = result.token;
        return;
      }
      onSuccess(result.session.user, result.session.firstLogin);
    } catch (cause) {
      error = cause instanceof ApiError ? "Sign-in failed" : "The server could not be reached";
    } finally {
      busy = false;
    }
  }
</script>

<Card class="w-full max-w-md p-6">
  <form class="space-y-4" onsubmit={submit}>
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Nimbus</h1>
      <p class="text-sm text-muted-foreground">Sign in to your files.</p>
    </div>

    {#if pendingToken}
      <div class="space-y-2">
        <Label for="otp">Authenticator code</Label>
        <Input id="otp" bind:value={otp} inputmode="numeric" autocomplete="one-time-code" maxlength={6} />
      </div>
    {:else}
      <div class="space-y-2">
        <Label for="username">Username</Label>
        <Input id="username" bind:value={username} autocomplete="username" />
      </div>
      <div class="space-y-2">
        <Label for="password">Password</Label>
        <Input id="password" type="password" bind:value={password} autocomplete="current-password" />
      </div>
      <label class="flex items-center gap-2 text-sm">
        <input type="checkbox" bind:checked={remember} />
        Remember this session
      </label>
    {/if}

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}

    <Button type="submit" class="w-full" disabled={busy}>
      {pendingToken ? "Verify" : "Sign in"}
    </Button>
  </form>
</Card>
