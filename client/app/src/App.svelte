<script lang="ts">
  import { NimbusClient } from "@nimbus/shared/api";
  import "@nimbus/shared/router";
  import { session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { onMount } from "svelte";
  import { loadEndpoint, saveEndpoint } from "./settings";

  let ready = $state(false);

  onMount(async () => {
    const endpoint = await loadEndpoint();

    session.native = true;
    session.endpoint = endpoint;
    session.client = new NimbusClient(endpoint ?? "");

    session.configureEndpoint = async (url: string) => {
      const trimmed = url.trim().replace(/\/$/, "");
      if (!trimmed) {
        throw new Error("empty endpoint");
      }
      await saveEndpoint(trimmed);
      session.endpoint = trimmed;
      session.client = new NimbusClient(trimmed);
    };

    ready = true;
  });
</script>

<SvelteToast options={toastOptions} />

{#if ready}
  <Router />
{:else}
  <p class="p-6 text-sm text-muted-foreground">Loading client settings…</p>
{/if}
