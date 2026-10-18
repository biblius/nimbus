<script lang="ts">
  import "@nimbus/shared/router";
  import { clearSession, session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { onMount } from "svelte";
  import {
    apiBaseUrl,
    loadEndpoint,
    normalizeOrigin,
    saveEndpoint,
  } from "./settings";
  import { NimbusClient } from "@nimbus/shared/api";

  let ready = $state(false);

  onMount(async () => {
    session.native = true;

    const loaded = await loadEndpoint();
    const origin = loaded ? normalizeOrigin(loaded) : null;

    if (loaded && origin && loaded !== origin) {
      await saveEndpoint(origin);
    }

    session.endpoint = origin;

    session.client = new NimbusClient(origin ? apiBaseUrl(origin) : "", {
      onUnauthorized: clearSession,
    });

    session.configureEndpoint = async (url: string) => {
      const normalized = normalizeOrigin(url);

      if (!normalized) {
        throw new Error("empty endpoint");
      }
      await saveEndpoint(normalized);

      session.endpoint = normalized;

      session.client = new NimbusClient(apiBaseUrl(normalized), {
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
