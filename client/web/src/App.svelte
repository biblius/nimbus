<script lang="ts">
  import { NimbusClient } from "@nimbus/shared/api";
  import SessionApp from "@nimbus/shared/components/SessionApp.svelte";

  const client = new NimbusClient(
    import.meta.env.VITE_NIMBUS_URL ?? "http://localhost:42069",
  );

  function emailedResetToken(): string | undefined {
    const path = window.location.pathname;
    for (const prefix of ["/password-reset/", "/reset/"]) {
      if (!path.startsWith(prefix)) continue;
      const token = decodeURIComponent(path.slice(prefix.length).split("/")[0]);
      return token || undefined;
    }
    return undefined;
  }

  const resetToken = emailedResetToken();
</script>

<SessionApp {client} {resetToken} />
