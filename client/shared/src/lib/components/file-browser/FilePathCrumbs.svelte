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
  import { cn } from "../../utils";

  let {
    crumbs,
    onNavigate,
    class: className,
  }: {
    crumbs: Crumb[];
    /** Called when an ancestor crumb is clicked (never the current page). */
    onNavigate: (index: number, crumb: Crumb) => void;
    class?: string;
  } = $props();
</script>

<nav
  class={cn(
    "flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto text-sm [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
    className,
  )}
  aria-label="Current folder"
>
  {#each crumbs as crumb, index (crumb.id ?? "root")}
    {#if index > 0}
      <ChevronRight
        size={14}
        class="shrink-0 text-muted-foreground/70"
        aria-hidden="true"
      />
    {/if}

    {#if index === crumbs.length - 1}
      <span
        class="shrink-0 truncate px-1.5 py-1 font-medium"
        aria-current="page"
      >
        {crumb.name}
      </span>
    {:else}
      <button
        type="button"
        class="shrink-0 truncate rounded-md px-1.5 py-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        onclick={() => onNavigate(index, crumb)}
      >
        {crumb.name}
      </button>
    {/if}
  {/each}
</nav>
