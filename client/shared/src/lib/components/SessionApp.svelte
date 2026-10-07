<script lang="ts">
  import { onMount } from "svelte";
  import { ApiError, type NimbusClient } from "../api";
  import type { Folders, Root, UserView } from "../types";
  import FileBrowser from "./FileBrowser.svelte";
  import LoginPanel from "./LoginPanel.svelte";
  import Button from "./ui/button/button.svelte";
  import Input from "./ui/input/input.svelte";

  let { client }: { client: NimbusClient } = $props();

  let user = $state<UserView | null>(null);
  let firstLogin = $state(false);
  let folders = $state<Folders | null>(null);
  let active = $state<Root | null>(null);
  let currentPassword = $state("");
  let newPassword = $state("");
  let notice = $state("");
  let ready = $state(false);

  async function loadSpace(profile: UserView) {
    user = profile;
    folders = await client.folders();
    active = folders.privateFolder.root;
  }

  onMount(async () => {
    try {
      await loadSpace(await client.me());
    } catch {
      user = null;
    } finally {
      ready = true;
    }
  });

  async function signedIn(profile: UserView, initial: boolean) {
    firstLogin = initial;
    notice = "";
    await loadSpace(profile);
  }

  async function logout() {
    await client.logout();
    user = null;
    folders = null;
    active = null;
  }

  async function changePassword(event: SubmitEvent) {
    event.preventDefault();
    notice = "";
    try {
      await client.changePassword(currentPassword, newPassword);
      currentPassword = "";
      newPassword = "";
      firstLogin = false;
      notice = "Password updated.";
    } catch (cause) {
      notice =
        cause instanceof ApiError
          ? "Password was not changed"
          : "The server could not be reached";
    }
  }
</script>

{#if !ready}
  <p class="p-8 text-sm text-muted-foreground">Loading…</p>
{:else if !user || !folders || !active}
  <main class="flex min-h-screen items-center justify-center p-6">
    <LoginPanel {client} onSuccess={signedIn} />
  </main>
{:else}
  <main class="mx-auto flex min-h-screen max-w-5xl flex-col gap-6 p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-sm text-muted-foreground">Signed in as</p>
        <h1 class="text-xl font-semibold">{user.username}</h1>
        <p class="text-sm text-muted-foreground">{user.role}</p>
      </div>
      <Button variant="outline" onclick={logout}>Log out</Button>
    </header>

    {#if firstLogin}
      <form
        class="space-y-3 rounded-xl border bg-card p-4"
        onsubmit={changePassword}
      >
        <p class="text-sm">
          This account is still using the password an administrator assigned.
          Change it.
        </p>
        <Input
          type="password"
          bind:value={currentPassword}
          placeholder="Current password"
        />
        <Input
          type="password"
          bind:value={newPassword}
          placeholder="New password"
        />
        <Button type="submit">Update password</Button>
      </form>
    {/if}

    {#if notice}
      <p class="text-sm">{notice}</p>
    {/if}

    <nav class="flex flex-wrap gap-2">
      <Button
        variant={active.id === folders.privateFolder.root.id
          ? "default"
          : "outline"}
        onclick={() => (active = folders?.privateFolder.root ?? active)}
      >
        Private
      </Button>
      {#each folders.shared as space (space.root.id)}
        <Button
          variant={active.id === space.root.id ? "default" : "outline"}
          onclick={() => (active = space.root)}
        >
          {space.root.name}
        </Button>
      {/each}
    </nav>

    <FileBrowser {client} folder={active} />
  </main>
{/if}
