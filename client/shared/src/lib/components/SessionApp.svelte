<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { isActive, navigate } from "../router";
  import { logout, session } from "../session.svelte";
  import AppHeader from "./AppHeader.svelte";

  let { children }: { children: Snippet } = $props();

  const admin = $derived(isActive.startsWith("/admin"));

  onMount(async () => {
    try {
      session.user = await session.client.me();
    } catch {
      session.user = null;
      navigate("/login", { replace: true });
    }
  });
</script>

{#if session.user}
  <div class="flex min-h-dvh flex-col">
    <AppHeader
      user={session.user}
      mode={admin ? "admin" : "user"}
      menuOpen={session.menuOpen}
      onToggleMenu={() => (session.menuOpen = !session.menuOpen)}
      onToggleMode={() => {
        session.menuOpen = false;
        navigate(admin ? "/files" : "/admin/users");
      }}
      onLogout={logout}
    />
    {@render children()}
  </div>
{:else}
  <p class="p-8 text-sm text-muted-foreground">Loading…</p>
{/if}
