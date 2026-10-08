<script lang="ts">
  import { NimbusClient } from "@nimbus/shared/api";
  import "@nimbus/shared/router";
  import { session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { onMount } from "svelte";
  import { loadEndpoint, saveEndpoint } from "./settings";

  let endpoint = $state("http://localhost:42069");
  let draft = $state("http://localhost:42069");
  let client = $state<NimbusClient | null>(null);

  onMount(async () => {
    endpoint = await loadEndpoint();
    draft = endpoint;
    session.client = new NimbusClient(endpoint);
    client = session.client;
  });

  async function apply(event: SubmitEvent) {
    event.preventDefault();
    endpoint = draft.trim();
    await saveEndpoint(endpoint);
    session.client = new NimbusClient(endpoint);
    client = session.client;
  }
</script>

<div class="border-b bg-card px-6 py-3">
  <form class="mx-auto flex max-w-5xl items-center gap-2" onsubmit={apply}>
    <label class="text-sm text-muted-foreground" for="endpoint">Server</label>
    <input
      id="endpoint"
      class="h-9 flex-1 rounded-md border bg-background px-3 text-sm"
      bind:value={draft}
    />
    <button
      class="h-9 rounded-md bg-primary px-3 text-sm text-primary-foreground"
      type="submit"
    >
      Save
    </button>
  </form>
</div>

<SvelteToast options={toastOptions} />

{#if client}
  {#key endpoint}
    <Router />
  {/key}
{:else}
  <p class="p-6 text-sm text-muted-foreground">Loading client settings…</p>
{/if}
