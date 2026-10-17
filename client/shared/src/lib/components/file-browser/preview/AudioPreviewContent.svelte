<script lang="ts">
  import { Music } from "@lucide/svelte";

  let {
    src,
    name,
    autoplay = true,
    /** Omit on public shared media; use `"use-credentials"` when auth cookies are required. */
    crossorigin,
  }: {
    src: string;
    name: string;
    autoplay?: boolean;
    crossorigin?: "anonymous" | "use-credentials";
  } = $props();

  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    void src;
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
      {src}
      controls
      {autoplay}
      {crossorigin}
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
