<script lang="ts">
  import { Music } from "@lucide/svelte";
  import { session } from "../../session.svelte";

  let {
    rootId,
    fileId,
    name,
    autoplay = true,
  }: {
    rootId: string;
    fileId: string;
    name: string;
    autoplay?: boolean;
  } = $props();

  // Direct chunk URL — browser issues Range requests and streams.
  const audioUrl = $derived(session.client.contentUrl(rootId, fileId));

  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    void fileId;
    loading = true;
    error = "";
  });
</script>

{#if error}
  <p class="text-sm text-red-300">{error}</p>
{:else}
  <div
    class="relative flex w-full max-w-md flex-col items-center gap-6 rounded-2xl bg-white/8 px-6 py-8 ring-1 ring-white/12"
  >
    {#if loading}
      <div
        class="absolute size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
        aria-label="Loading"
      ></div>
    {/if}
    <Music
      size={40}
      strokeWidth={1.5}
      class={loading ? "text-white/55 opacity-0" : "text-white/55"}
      aria-hidden="true"
    />
    <audio
      src={audioUrl}
      controls
      {autoplay}
      preload="auto"
      aria-label={name}
      class="w-full max-w-md"
      class:opacity-0={loading}
      onloadstart={() => {
        loading = false;
      }}
      onerror={() => {
        error = "Could not load audio";
        loading = false;
      }}
    ></audio>
  </div>
{/if}
