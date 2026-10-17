<script lang="ts" module>
  /**
   * A path segment in the folder trail.
   * A crumb with `id === null` represents the root directory.
   */
  export type Crumb = {
    id: string | null;
    name: string;
  };
</script>

<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";

  let {
    crumbs,
    onNavigate,
  }: {
    crumbs: Crumb[];
    /** Called when an ancestor crumb is clicked (never the current page). */
    onNavigate: (index: number, crumb: Crumb) => void;
  } = $props();
</script>

<nav
  class="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto text-sm"
  aria-label="Current folder"
>
  {#each crumbs as crumb, index (crumb.id ?? "root")}
    {#if index > 0}
      <ChevronRight
        size={14}
        class="shrink-0 text-muted-foreground"
        aria-hidden="true"
      />
    {/if}

    {#if index === crumbs.length - 1}
      <span
        class="max-w-40 truncate px-1.5 py-1 font-medium sm:max-w-none"
        aria-current="page"
      >
        {crumb.name}
      </span>
    {:else}
      <button
        type="button"
        class="max-w-40 truncate rounded-md px-1.5 py-1 text-muted-foreground hover:bg-accent hover:text-foreground sm:max-w-none"
        onclick={() => onNavigate(index, crumb)}
      >
        {crumb.name}
      </button>
    {/if}
  {/each}
</nav>
