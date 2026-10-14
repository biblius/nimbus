<script lang="ts">
  import { session } from "../../session.svelte";

  let {
    rootId,
    fileId,
    name,
  }: {
    rootId: string;
    fileId: string;
    name: string;
  } = $props();

  // Direct chunk URL — browser issues Range requests and streams.
  const videoUrl = $derived(session.client.contentUrl(rootId, fileId));

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
  <div class="relative flex max-h-full max-w-full items-center justify-center">
    {#if loading}
      <div
        class="absolute size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
        aria-label="Loading"
      ></div>
    {/if}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      src={videoUrl}
      controls
      playsinline
      preload="metadata"
      aria-label={name}
      class="max-h-full max-w-full shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)]"
      class:opacity-0={loading}
      onloadedmetadata={() => {
        loading = false;
      }}
      onerror={() => {
        error = "Could not load video";
        loading = false;
      }}
    ></video>
  </div>
{/if}
