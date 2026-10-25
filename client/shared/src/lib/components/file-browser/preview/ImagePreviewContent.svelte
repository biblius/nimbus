<script lang="ts">
  let { load, name }: { load: () => Promise<Blob>; name: string } = $props();

  let imageUrl = $state<string | null>(null);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    const loadBlob = load;
    const fileName = name;

    imageUrl = null;
    error = "";
    loading = true;

    let cancelled = false;
    let objectUrl: string | null = null;

    loadBlob()
      .then((blob) => {
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(blob);
        imageUrl = objectUrl;
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = `Could not load ${fileName}`;
        loading = false;
      });

    return () => {
      cancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  });
</script>

{#if loading}
  <div
    class="size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
    aria-label="Loading"
  ></div>
{:else if error}
  <p class="text-sm text-red-300">{error}</p>
{:else if imageUrl}
  <img
    src={imageUrl}
    alt={name}
    class="max-h-full max-w-full object-contain shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] select-none"
    draggable="false"
  />
{/if}
