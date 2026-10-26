<script lang="ts">
  import { onMount } from "svelte";
  import ConfigForm from "../../components/admin/ConfigForm.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type {
    NimbusConfigUpdate,
    NimbusConfiguration,
  } from "../../types";

  let config = $state<NimbusConfiguration | null>(null);
  let loadFailed = $state(false);

  onMount(async () => {
    try {
      config = await session.client.getConfig();
    } catch (cause) {
      loadFailed = true;
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load settings",
      );
    }
  });

  async function save(
    input: NimbusConfigUpdate,
  ): Promise<NimbusConfiguration> {
    return session.client.updateConfig(input);
  }
</script>

<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
  <div class="mx-auto max-w-3xl">
    {#if config}
      <ConfigForm bind:config {save} />
    {:else}
      <div class="border-b pb-3">
        <h2 class="text-lg font-semibold tracking-tight">Settings</h2>
        <p class="mt-0.5 text-sm text-muted-foreground">
          {#if loadFailed}
            Could not load configuration. Refresh the page to try again.
          {:else}
            Loading configuration…
          {/if}
        </p>
      </div>
    {/if}
  </div>
</div>
