<script lang="ts">
  import { navigate } from "../router";
  import { session } from "../session.svelte";
  import type { Root } from "../types";

  let {
    roots,
    selected,
  }: {
    roots: Root[];
    selected?: Root;
  } = $props();

  const priv = $derived(roots.filter((root) => root.owner != null));
  const shared = $derived(roots.filter((root) => root.owner == null));
</script>

<nav class="flex flex-col gap-1 p-3" aria-label="Roots">
  <p
    class="hidden px-3 pb-2 text-center text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
  >
    Folders
  </p>
  {#if roots.length === 0}
    <p class="px-3 text-sm text-muted-foreground">No roots are assigned.</p>
  {:else}
    {#each priv as root (root.id)}
      <button
        type="button"
        class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {selected?.id ===
        root.id
          ? 'bg-accent font-medium text-accent-foreground'
          : 'hover:bg-accent'}"
        aria-current={selected?.id === root.id ? "page" : undefined}
        onclick={() => {
          session.menuOpen = false;
          navigate("/files/:root_id", { params: { root_id: root.id } });
        }}
      >
        <span class="truncate">{root.name}</span>
      </button>
    {/each}
    <div
      class="mt-2 flex flex-col gap-1 border-t pt-2"
      role="group"
      aria-labelledby="shared-roots-heading"
    >
      <p
        id="shared-roots-heading"
        class="px-1.5 py-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase"
      >
        Shared
      </p>
      {#each shared as root (root.id)}
        <button
          type="button"
          class="flex min-h-11 items-center rounded-md pr-3 pl-6 text-left text-sm {selected?.id ===
          root.id
            ? 'bg-accent font-medium text-accent-foreground'
            : 'hover:bg-accent'}"
          aria-current={selected?.id === root.id ? "page" : undefined}
          onclick={() => {
            session.menuOpen = false;
            navigate("/files/:root_id", { params: { root_id: root.id } });
          }}
        >
          <span class="truncate">{root.name}</span>
        </button>
      {/each}
    </div>
  {/if}
</nav>
