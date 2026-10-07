<script lang="ts">
  import type { NimbusClient } from "../../api";
  import type { Root } from "../../types";
  import FileBrowser from "../FileBrowser.svelte";
  import PanelShell from "../PanelShell.svelte";

  let {
    client,
    roots,
    active = $bindable(),
    menuOpen = $bindable(false),
    admin,
    onEditRoot,
  }: {
    client: NimbusClient;
    roots: Root[];
    active: Root | null;
    menuOpen?: boolean;
    admin: boolean;
    onEditRoot: (root: Root) => void;
  } = $props();

  function choose(root: Root) {
    active = root;
    menuOpen = false;
  }
</script>

<PanelShell bind:open={menuOpen} label="Roots">
  {#snippet sidebar()}
    <nav class="flex flex-col gap-1 p-3" aria-label="Roots">
      <p
        class="hidden px-3 pb-2 text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
      >
        Roots
      </p>
      {#if roots.length === 0}
        <p class="px-3 text-sm text-muted-foreground">No roots are assigned.</p>
      {:else}
        {#each roots as root (root.id)}
          <button
            type="button"
            class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {active?.id ===
            root.id
              ? 'bg-accent font-medium text-accent-foreground'
              : 'hover:bg-accent'}"
            aria-current={active?.id === root.id ? "page" : undefined}
            onclick={() => choose(root)}
          >
            <span class="truncate">{root.name}</span>
          </button>
        {/each}
      {/if}
    </nav>
  {/snippet}

  <div class="p-4 sm:p-6">
    <div class="mx-auto max-w-5xl">
      {#if active}
        {@const folder = active}
        {#key folder.id}
          <FileBrowser
            {client}
            {folder}
            {admin}
            onEditRoot={() => onEditRoot(folder)}
          />
        {/key}
      {:else}
        <p class="rounded-xl border bg-card p-4 text-sm text-muted-foreground">
          No storage roots are assigned to this account.
        </p>
      {/if}
    </div>
  </div>
</PanelShell>
