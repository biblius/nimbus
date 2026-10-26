<script lang="ts">
  import { initRouter } from "@nimbus/shared/router";
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
  import { listenNativeFileDrop } from "./native-file-drop";
  import { pickNativeFiles, pickNativeFolder } from "./native-file-pick";
  import { setSessionCookie } from "./session-cookie";
  import UpdatePrompt from "./UpdatePrompt.svelte";

  // Native never registers public share viewer routes.
  initRouter();

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

    // Rotate the persisted Bearer on every launch, then mirror it into the
    // webview cookie jar so chunked media streams authenticate without CORS.
    if (origin && accessToken) {
      try {
        await session.client.refreshSession();
      } catch (error) {
        console.warn("failed to rotate session", error);
        // Offline/transient failure: keep the old Bearer. A 401 already
        // cleared storage via onUnauthorized — don't revive a dead cookie.
        const still = await loadAccessToken();
        if (still) {
          try {
            await setSessionCookie(origin, still);
          } catch (cookieError) {
            console.warn("failed to restore session cookie", cookieError);
          }
        }
      }
    }

    session.configureEndpoint = async (url: string) => {
      const normalized = parseServerOrigin(url);

      if (!normalized) {
        throw new Error("invalid endpoint");
      }
      await saveEndpoint(normalized);

      session.endpoint = normalized;

      const token = await loadAccessToken();
      // Rebuild so setAccessToken/syncSessionCookie targets the new origin.
      session.client = buildClient(apiBaseUrl(normalized), token);
      if (token) {
        try {
          await setSessionCookie(normalized, token);
        } catch (error) {
          console.warn("failed to restore session cookie", error);
        }
      }
    };

    const { openUrl } = await import("@tauri-apps/plugin-opener");
    session.openExternalUrl = (url) => openUrl(url);
    session.listenFileDrop = listenNativeFileDrop;
    session.pickFiles = pickNativeFiles;
    session.pickFolder = pickNativeFolder;

    ready = true;
  });
</script>

<SvelteToast options={toastOptions} />
<UpdatePrompt />

{#if ready}
  <Router />
{:else}
  <p class="p-6 text-sm text-muted-foreground">Loading client settings…</p>
{/if}
