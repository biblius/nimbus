<script lang="ts">
  import type { Snippet } from "svelte";
  import AdminSidebar from "../../components/AdminSidebar.svelte";
  import PanelShell from "../../components/PanelShell.svelte";
  import { navigate } from "../../router";
  import { session } from "../../session.svelte";

  let { children }: { children: Snippet } = $props();

  $effect(() => {
    // Ensure non-admins get redirected back to the users page
    if (session.user?.role !== "admin") {
      navigate("/files", { replace: true });
    }
  });
</script>

{#if session.user?.role === "admin"}
  <PanelShell bind:open={session.menuOpen} label="Configuration">
    {#snippet sidebar()}
      <AdminSidebar />
    {/snippet}

    <!-- PAGE CONTENT -->

    <div class="p-4 sm:p-6">
      <div class="mx-auto max-w-5xl">
        {@render children()}
      </div>
    </div>
  </PanelShell>
{/if}
