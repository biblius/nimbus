<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    // Handles the open state of the sidebar on small screens
    open = $bindable(false),
    label,
    sidebar,
    children,
  }: {
    open?: boolean;
    label: string;
    sidebar: Snippet;
    children: Snippet;
  } = $props();
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape" && open) {
      open = false;
    }
  }}
/>

<div class="flex min-h-0 flex-1">
  {#if open}
    <button
      type="button"
      class="fixed inset-0 z-30 bg-foreground/40 md:hidden"
      aria-label="Close menu"
      onclick={() => (open = false)}
    ></button>
  {/if}

  <aside
    class="fixed top-14 bottom-0 left-0 z-40 flex w-72 max-w-[85vw] flex-col overflow-y-auto border-r bg-card transition-transform duration-200 md:sticky md:top-14 md:bottom-auto md:z-auto md:h-[calc(100dvh-3.5rem)] md:w-64 md:max-w-none md:translate-x-0 {open
      ? 'translate-x-0'
      : '-translate-x-full'}"
    aria-label={label}
  >
    <div class="flex items-center justify-between px-3 pt-3 md:hidden">
      <p class="text-sm font-medium">{label}</p>
      <button
        type="button"
        class="inline-flex h-11 items-center rounded-md px-3 text-sm hover:bg-accent"
        onclick={() => (open = false)}
      >
        Close
      </button>
    </div>
    {@render sidebar()}
  </aside>

  <div class="min-h-0 min-w-0 flex-1 overflow-y-auto">
    {@render children()}
  </div>
</div>
