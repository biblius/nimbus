<script lang="ts">
  import PanelShell from "../PanelShell.svelte";
  import { ADMIN_SIDEBAR_ENTRIES, type AdminContext } from "./categories";
  import FoldersPage from "./FoldersPage.svelte";
  import UsersPage from "./UsersPage.svelte";

  let {
    menuOpen = $bindable(false),
    context,
  }: {
    menuOpen?: boolean;
    context: AdminContext;
  } = $props();

  let selectedEntry: string = $state(ADMIN_SIDEBAR_ENTRIES[0].id);

  const active = $derived(
    ADMIN_SIDEBAR_ENTRIES.find((entry) => entry.id === selectedEntry) ??
      ADMIN_SIDEBAR_ENTRIES[0],
  );

  function choose(id: string) {
    selectedEntry = id;
    menuOpen = false;
  }
</script>

<PanelShell bind:open={menuOpen} label="Configuration">
  {#snippet sidebar()}
    <nav class="flex flex-col gap-1 p-3" aria-label="Configuration">
      <p
        class="hidden px-3 pb-2 text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
      >
        Configuration
      </p>
      {#each ADMIN_SIDEBAR_ENTRIES as entry (entry.id)}
        <button
          type="button"
          class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {active.id ===
          entry.id
            ? 'bg-accent font-medium text-accent-foreground'
            : 'hover:bg-accent'}"
          aria-current={active.id === entry.id ? "page" : undefined}
          onclick={() => choose(entry.id)}
        >
          {entry.label}
        </button>
      {/each}
    </nav>
  {/snippet}

  <div class="p-4 sm:p-6">
    <div class="mx-auto max-w-5xl">
      {#if active.id === "users"}
        <UsersPage {context} />
      {:else if active.id === "folders"}
        <FoldersPage {context} />
      {/if}
    </div>
  </div>
</PanelShell>
