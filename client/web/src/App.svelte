<script lang="ts">
  import { createClient } from "@nimbus/shared/api";
  import SessionApp from "@nimbus/shared/components/SessionApp.svelte";

  const client = createClient(
    import.meta.env.VITE_NIMBUS_URL ?? "http://localhost:42069",
  );
</script>

<SessionApp {client} />
