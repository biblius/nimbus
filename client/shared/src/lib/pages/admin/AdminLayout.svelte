<script lang="ts">
  import type { Snippet } from "svelte";
  import AdminSidebar from "../../components/AdminSidebar.svelte";
  import DeletionBanner from "../../components/DeletionBanner.svelte";
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

    <!-- Fill under the app header; pages scroll internally like the file browser. -->
    <div
      class="flex h-[calc(100dvh-3.5rem)] min-h-0 flex-col overflow-hidden p-4 sm:p-6"
    >
      <div class="mx-auto flex min-h-0 w-full max-w-5xl flex-1 flex-col">
        {@render children()}
      </div>
    </div>
  </PanelShell>

  <DeletionBanner />
{/if}
