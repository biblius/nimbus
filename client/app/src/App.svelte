<script lang="ts">
  import "@nimbus/shared/router";
  import { clearSession, session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { onMount } from "svelte";
  import { loadEndpoint, saveEndpoint } from "./settings";
  import { NimbusClient } from "@nimbus/shared/api";

  let ready = $state(false);

  onMount(async () => {
    session.native = true;

    const endpoint = await loadEndpoint();

    session.endpoint = endpoint;

    session.client = new NimbusClient(endpoint ?? "", {
      onUnauthorized: clearSession,
    });

    session.configureEndpoint = async (url: string) => {
      const trimmed = url.trim().replace(/\/$/, "");

      if (!trimmed) {
        throw new Error("empty endpoint");
      }
      await saveEndpoint(trimmed);
      session.endpoint = trimmed;
      session.client = new NimbusClient(trimmed, {
        onUnauthorized: clearSession,
      });
    };

    const { openUrl } = await import("@tauri-apps/plugin-opener");
    session.openExternalUrl = (url) => openUrl(url);

    ready = true;
  });
</script>

<SvelteToast options={toastOptions} />

{#if ready}
  <Router />
{:else}
  <p class="p-6 text-sm text-muted-foreground">Loading client settings…</p>
{/if}
