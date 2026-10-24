<script lang="ts">
  import "@nimbus/shared/router";
  import { clearSession, session } from "@nimbus/shared/session";
  import { parseServerOrigin } from "@nimbus/shared/server-url";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { onMount } from "svelte";
  import {
    apiBaseUrl,
    clearAccessToken,
    loadAccessToken,
    loadEndpoint,
    saveAccessToken,
    saveEndpoint,
  } from "./settings";
  import { NimbusClient } from "./api";

  let ready = $state(false);

  function buildClient(baseUrl: string, accessToken: string | null) {
    return new NimbusClient(baseUrl, {
      accessToken,
      onUnauthorized: clearSession,
      onAccessTokenChange: async (token) => {
        if (token) {
          await saveAccessToken(token);
        } else {
          await clearAccessToken();
        }
      },
    });
  }

  onMount(async () => {
    session.native = true;

    const origin = await loadEndpoint();
    session.endpoint = origin;

    const accessToken = await loadAccessToken();

    session.client = buildClient(
      origin ? apiBaseUrl(origin) : "",
      accessToken,
    );

    session.configureEndpoint = async (url: string) => {
      const normalized = parseServerOrigin(url);

      if (!normalized) {
        throw new Error("invalid endpoint");
      }
      await saveEndpoint(normalized);

      session.endpoint = normalized;

      const token = await loadAccessToken();
      session.client = buildClient(apiBaseUrl(normalized), token);
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
