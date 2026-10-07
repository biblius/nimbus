<script lang="ts">
  import { onMount } from "svelte";
  import { NimbusClient } from "../api";
  import type { Root, UserView } from "../types";
  import AdminPanel from "./admin/AdminPanel.svelte";
  import type { AdminContext, EditRequest } from "./admin/categories";
  import AppHeader from "./AppHeader.svelte";
  import LoginPanel from "./LoginPanel.svelte";
  import PasswordResetPanel from "./PasswordResetPanel.svelte";
  import UserPanel from "./user/UserPanel.svelte";

  let {
    client,
    resetToken,
  }: {
    client: NimbusClient;
    resetToken?: string;
  } = $props();

  let user = $state<UserView | null>(null);
  let roots = $state<Root[]>([]);
  let active = $state<Root | null>(null);
  let ready = $state(false);
  let mode = $state<"user" | "admin">("user");
  let menuOpen = $state(false);
  let editRequest = $state<EditRequest | null>(null);
  let requestedReset = $state(false);
  let dismissedReset = $state(false);

  const showReset = $derived(
    !dismissedReset && (requestedReset || resetToken !== undefined),
  );
  const activeResetToken = $derived(requestedReset ? undefined : resetToken);

  const adminContext = $derived.by((): AdminContext | null => {
    if (!user) return null;
    return {
      client,
      editRequest,
      currentUserId: user.id,
      onAccountChanged,
      onAccountDeleted,
    };
  });

  async function loadSpace(profile: UserView) {
    user = profile;
    roots = await client.roots();
    active = roots.find((root) => root.primary) ?? roots[0] ?? null;
  }

  async function refreshRoots() {
    const next = await client.roots();
    roots = next;
    const current = active?.id;
    active =
      next.find((root) => root.id === current) ??
      next.find((root) => root.primary) ??
      next[0] ??
      null;
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

  async function signedIn(profile: UserView) {
    mode = "user";
    menuOpen = false;
    dismissedReset = true;
    requestedReset = false;
    await loadSpace(profile);
  }

  async function logout() {
    await client.logout();
    user = null;
    roots = [];
    active = null;
    mode = "user";
    menuOpen = false;
    editRequest = null;
  }

  async function toggleMode() {
    if (user?.role !== "admin") return;

    menuOpen = false;

    if (mode === "admin") {
      await refreshRoots();
      mode = "user";
      return;
    }
    mode = "admin";
  }

  function editRoot(root: Root) {
    editRequest = { id: root.id, token: (editRequest?.token ?? 0) + 1 };
    mode = "admin";
    menuOpen = false;
  }

  function onAccountChanged(account: UserView) {
    if (!user || user.id !== account.id) return;
    user = {
      ...user,
      username: account.username,
      email: account.email,
      role: account.role,
    };
    if (account.role !== "admin") mode = "user";
  }

  function onAccountDeleted(id: string) {
    if (user?.id === id) void logout();
  }

  function requestReset() {
    requestedReset = true;
    dismissedReset = false;
  }

  function closeReset() {
    requestedReset = false;
    dismissedReset = true;
  }
</script>

{#if showReset}
  <main class="flex min-h-screen items-center justify-center p-6">
    <PasswordResetPanel {client} token={activeResetToken} onBack={closeReset} />
  </main>
{:else if !ready}
  <p class="p-8 text-sm text-muted-foreground">Loading…</p>
{:else if !user || !adminContext}
  <main class="flex min-h-screen items-center justify-center p-6">
    <LoginPanel {client} onSuccess={signedIn} onResetPassword={requestReset} />
  </main>
{:else}
  <div class="flex min-h-dvh flex-col">
    <AppHeader
      {user}
      {mode}
      {menuOpen}
      onToggleMenu={() => (menuOpen = !menuOpen)}
      onToggleMode={toggleMode}
      onLogout={logout}
    />

    {#if mode === "admin" && user.role === "admin"}
      <AdminPanel bind:menuOpen context={adminContext} />
    {:else}
      <UserPanel
        {client}
        {roots}
        bind:active
        bind:menuOpen
        admin={user.role === "admin"}
        onEditRoot={editRoot}
      />
    {/if}
  </div>
{/if}
