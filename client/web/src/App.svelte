<script lang="ts">
  import { NimbusClient } from "@nimbus/shared/api";
  import "@nimbus/shared/router";
  import { session } from "@nimbus/shared/session";
  import { SvelteToast } from "@zerodevx/svelte-toast";
  import { toastOptions } from "@nimbus/shared/toast";
  import { Router } from "sv-router";

  session.client = new NimbusClient(
    import.meta.env.VITE_NIMBUS_URL ?? "http://localhost:42069",
  );
</script>

<SvelteToast options={toastOptions} />

<Router />
