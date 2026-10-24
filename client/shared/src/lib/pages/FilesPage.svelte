<script lang="ts">
  import { onMount } from "svelte";
  import ActivityBrowser from "../components/ActivityBrowser.svelte";
  import FileBrowser from "../components/file-browser/FileBrowser.svelte";
  import PanelShell from "../components/PanelShell.svelte";
  import TrashBrowser from "../components/TrashBrowser.svelte";
  import UserSidebar from "../components/UserSidebar.svelte";
  import { isActive, navigate, route } from "../router";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { Root } from "../types";

  let roots = $state<Root[]>([]);
  let loaded = $state(false);

  const rootId = $derived(route.params.root_id);
  const parentId = $derived(route.params.parent_id ?? null);
  const viewingTrash = $derived(isActive("/files/trash"));
  const viewingActivity = $derived(isActive("/files/:root_id/activity"));
  const selected = $derived(
    viewingTrash ? undefined : roots.find((root) => root.id === rootId),
  );

  onMount(async () => {
    try {
      roots = await session.client.roots();
    } catch {
      toastError("Could not load files");
    } finally {
      loaded = true;
    }
  });

  $effect(() => {
    if (
      viewingTrash ||
      viewingActivity ||
      rootId ||
      !loaded ||
      roots.length === 0
    ) {
      return;
    }

    const home =
      roots.find((root) => root.owner === session.user?.id) ?? roots[0];

    navigate("/files/:root_id", {
      params: { root_id: home.id },
      replace: true,
    });
  });
</script>

<PanelShell bind:open={session.menuOpen} label="Folders">
  {#snippet sidebar()}
    <UserSidebar {roots} {selected} />
  {/snippet}

  <!-- Fill the space under the app header. The file list scrolls inside. -->
  <div
    class="flex h-[calc(100dvh-3.5rem)] min-h-0 flex-col overflow-hidden p-4 sm:p-6"
  >
    <div class="mx-auto flex min-h-0 w-full max-w-7xl flex-1 flex-col">
      {#if viewingTrash && loaded}
        <TrashBrowser {roots} />
      {:else if viewingActivity && selected}
        {#key selected.id}
          <ActivityBrowser root={selected} />
        {/key}
      {:else if selected}
        <FileBrowser folder={selected} {parentId} {roots} />
      {:else if loaded && rootId}
        <p class="text-sm text-muted-foreground">This folder does not exist.</p>
      {/if}
    </div>
  </div>
</PanelShell>
