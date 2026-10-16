<script lang="ts">
  import "@nimbus/shared/router";
  import { clearSession, session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";
  import { NimbusClient } from "@nimbus/shared/api";

  const apiUrl = import.meta.env.VITE_NIMBUS_URL;
  if (typeof apiUrl !== "string" || !apiUrl) {
    throw new Error("VITE_NIMBUS_URL is not set");
  }

  session.client = new NimbusClient(apiUrl, {
    onUnauthorized: clearSession,
  });
</script>

<SvelteToast options={toastOptions} />

<Router />
