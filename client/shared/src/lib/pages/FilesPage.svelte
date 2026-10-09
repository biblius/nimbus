<script lang="ts">
  import { onMount } from "svelte";
  import FileBrowser from "../components/FileBrowser.svelte";
  import PanelShell from "../components/PanelShell.svelte";
  import { navigate } from "../router";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { Root } from "../types";

  let roots = $state<Root[]>([]);

  let selected = $state<Root>();

  onMount(async () => {
    try {
      roots = await session.client.roots();
      selected = roots.find((root) => root.owner === session.user?.id);
    } catch {
      toastError("Could not load files");
    }
  });
</script>

<PanelShell bind:open={session.menuOpen} label="Roots">
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
            class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {selected?.id ===
            root.id
              ? 'bg-accent font-medium text-accent-foreground'
              : 'hover:bg-accent'}"
            aria-current={selected?.id === root.id ? "page" : undefined}
            onclick={() => {
              selected = root;
              session.menuOpen = false;
            }}
          >
            <span class="truncate">{root.name}</span>
          </button>
        {/each}
      {/if}
    </nav>
  {/snippet}

  <div class="p-4 sm:p-6">
    <div class="mx-auto max-w-5xl">
      {#if selected}
        {@const folder = selected}
        {#key folder.id}
          <FileBrowser
            client={session.client}
            {folder}
            admin={session.user?.role === "admin"}
            onEditRoot={() =>
              navigate(
                folder.owner
                  ? "/admin/folders/private"
                  : "/admin/folders/shared",
                { search: { id: folder.id } },
              )}
          />
        {/key}
      {/if}
    </div>
  </div>
</PanelShell>
