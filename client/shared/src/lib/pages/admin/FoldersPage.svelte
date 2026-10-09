<script lang="ts">
  import { Pencil, RefreshCw, Trash } from "@lucide/svelte";
  import { onMount } from "svelte";
  import RootForm from "../../components/folder/RootForm.svelte";
  import RootInfo from "../../components/folder/RootInfo.svelte";
  import Button from "../../components/ui/button/button.svelte";
  import ConfirmDialog from "../../components/ui/confirm-dialog/confirm-dialog.svelte";
  import { formatBytes, formatQuota } from "../../format";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Member, Root, RootData, UserViewFull } from "../../types";

  let folders = $state<Root[]>([]);
  let users = $state<UserViewFull[]>([]);
  let kind = $state<"private" | "shared">("private");
  let selectedId = $state<string | null>(null);
  let members = $state<Member[]>([]);
  let membersLoaded = $state(false);
  let editing = $state(false);
  let creating = $state(false);
  let deleteFolder = $state<Root | null>(null);
  let deleting = $state(false);
  let refreshing = $state(false);
  /** Bytes stored in each root. Missing while the usage request is in flight; `null` if it failed. */
  let usage = $state<Record<string, number | null>>({});

  function reset() {
    selectedId = null;
    editing = false;
    creating = false;
    members = [];
    membersLoaded = false;
  }

  async function showMembers(id: string) {
    membersLoaded = false;

    const next = await session.client.listMembers(id);

    if (selectedId !== id) return;

    members = next;

    membersLoaded = true;
  }

  function ownerName(id: string | null) {
    if (!id) return "";
    return users.find((user) => user.id === id)?.username ?? "Owner";
  }

  function usageText(id: string) {
    if (!(id in usage)) return "…";

    const bytes = usage[id];

    return bytes == null ? "-" : formatBytes(bytes);
  }

  function loadUsage(roots: Root[]) {
    for (const folder of roots) {
      const id = folder.id;

      session.client
        .folderUsage(id)
        .then((bytes) => {
          usage[id] = bytes;
        })
        .catch(() => {
          usage[id] = null;
        });
    }
  }

  async function load() {
    const [roots, people] = await Promise.all([
      kind === "private"
        ? session.client.listPrivateFolders()
        : session.client.listSharedFolders(),
      session.client.listUsers(),
    ]);

    folders = roots;
    users = people;

    loadUsage(roots);

    if (selectedId && !roots.some((folder) => folder.id === selectedId)) {
      reset();
      return;
    }

    const selected = roots.find((folder) => folder.id === selectedId);
    if (!selected) return;
    if (selected.owner) {
      members = [];
      membersLoaded = true;
      return;
    }

    await showMembers(selected.id);
  }

  function choose(next: "private" | "shared") {
    if (kind === next) return;
    kind = next;
    reset();
    load().catch((cause) => {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load folders",
      );
    });
  }

  async function refresh() {
    if (refreshing) return;

    refreshing = true;

    try {
      await load();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load folders",
      );
    }
    refreshing = false;
  }

  onMount(() => {
    load().catch((cause) => {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not load folders",
      );
    });
  });

  async function createRoot(data: RootData) {
    const created = await session.client.createFolder(data);

    folders.unshift(created);
    usage[created.id] = 0;

    reset();
  }

  async function updateRoot(data: RootData, folder: Root) {
    const updated = await session.client.updateFolder(folder.id, data);

    folders = folders.filter((item) => item.id !== folder.id);

    folders.unshift(updated);

    members = users
      .filter((user) => data.members.includes(user.id))
      .map((user) => ({
        id: user.id,
        username: user.username,
        email: user.email,
        role: user.role,
        createdAt: user.createdAt,
      }));

    editing = false;
  }

  async function confirmDelete() {
    if (!deleteFolder || deleting) return;

    const folderId = deleteFolder.id;

    deleting = true;

    try {
      await session.client.deleteFolder(folderId);

      folders = folders.filter((folder) => folder.id !== folderId);

      deleteFolder = null;

      if (selectedId === folderId) reset();
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not remove the folder",
      );
    }
    deleting = false;
  }
</script>

<div class="space-y-3">
  <div
    class="flex items-center justify-between gap-3 rounded-lg border-b px-3 py-1.5"
  >
    <div class="flex items-center gap-1">
      <h2 class="text-sm font-medium">Folders</h2>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        class="size-9 p-0"
        aria-label="Refresh folders"
        title="Refresh"
        disabled={refreshing}
        onclick={refresh}
      >
        <RefreshCw size={16} class={refreshing ? "animate-spin" : ""} />
      </Button>
    </div>
    {#if kind === "shared"}
      <Button
        type="button"
        variant={creating ? "outline" : "ghost"}
        size="sm"
        class="size-9 p-0"
        aria-label={creating ? "Close new folder form" : "Add folder"}
        aria-expanded={creating}
        title="Add folder"
        onclick={() => {
          const next = !creating;
          reset();
          creating = next;
        }}
      >
        <svg
          viewBox="0 0 20 20"
          class="size-4 transition-transform {creating ? 'rotate-45' : ''}"
          fill="none"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <path d="M10 4v12M4 10h12" />
        </svg>
      </Button>
    {/if}
  </div>

  <div class="flex gap-1" role="tablist" aria-label="Folder kinds">
    <Button
      type="button"
      variant={kind === "private" ? "default" : "outline"}
      size="sm"
      role="tab"
      aria-selected={kind === "private"}
      onclick={() => choose("private")}
    >
      Private
    </Button>
    <Button
      type="button"
      variant={kind === "shared" ? "default" : "outline"}
      size="sm"
      role="tab"
      aria-selected={kind === "shared"}
      onclick={() => choose("shared")}
    >
      Shared
    </Button>
  </div>

  {#if creating}
    <div class="mx-auto max-w-lg">
      <RootForm
        title="Add folder"
        description="A quota of 0 means unlimited. Choose who can use this shared folder."
        {users}
        save={createRoot}
        cancel={reset}
      />
    </div>
  {/if}

  {#if folders.length === 0}
    <p class="text-sm text-muted-foreground">
      {kind === "private"
        ? "No private folders yet."
        : "No shared folders yet."}
    </p>
  {:else}
    <ul class="divide-y overflow-hidden rounded-lg border">
      {#each folders as folder (folder.id)}
        <li>
          <button
            type="button"
            class="flex min-h-11 w-full items-center justify-between gap-3 px-3 py-2 text-left {selectedId ===
            folder.id
              ? 'bg-accent'
              : 'hover:bg-accent/60'}"
            aria-expanded={selectedId === folder.id}
            onclick={() => {
              const next = selectedId === folder.id ? null : folder.id;
              reset();
              selectedId = next;
              if (next) {
                if (folder.owner) {
                  members = [];
                  membersLoaded = true;
                } else {
                  showMembers(next).catch(() => {
                    toastError("Could not load members");
                  });
                }
              }
            }}
          >
            <span class="min-w-0">
              <span class="block truncate text-sm font-medium"
                >{folder.name}</span
              >
              <span class="block truncate text-xs text-muted-foreground">
                {#if folder.owner}
                  {ownerName(folder.owner)} ·
                {/if}
                {usageText(folder.id)} / {formatQuota(folder.quota)}
              </span>
            </span>

            <svg
              viewBox="0 0 20 20"
              class="size-4 shrink-0 text-muted-foreground transition-transform {selectedId ===
              folder.id
                ? 'rotate-180'
                : ''}"
              fill="none"
              stroke="currentColor"
              stroke-width="1.75"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="m5 8 5 5 5-5" />
            </svg>
          </button>

          {#if selectedId === folder.id}
            <div class="border-t bg-muted/30 p-3">
              {#if editing}
                <div class="mx-auto max-w-lg">
                  <RootForm
                    {folder}
                    {users}
                    {members}
                    title="Edit folder"
                    description={folder.owner
                      ? "A private folder stays with its owner."
                      : "Choose who can use this shared folder."}
                    save={(data) => updateRoot(data, folder)}
                    cancel={() => (editing = false)}
                  />
                </div>
              {:else}
                <div class="flex justify-between gap-3">
                  <div class="min-w-0 flex-1 space-y-4">
                    <!-- ROOT CARD -->

                    <RootInfo
                      {folder}
                      usage={usageText(folder.id)}
                      ownerName={ownerName(folder.owner)}
                    />

                    {#if !folder.owner}
                      <div>
                        <p class="text-sm font-medium">Members</p>
                        {#if !membersLoaded}
                          <p class="text-sm text-muted-foreground">Loading…</p>
                        {:else if members.length === 0}
                          <p class="text-sm text-muted-foreground">
                            No members.
                          </p>
                        {:else}
                          <ul class="mt-1 space-y-1">
                            {#each members as member (member.id)}
                              <li class="truncate text-sm">
                                {member.username}
                              </li>
                            {/each}
                          </ul>
                        {/if}
                      </div>
                    {/if}
                  </div>
                  <div class="flex flex-col self-stretch">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      class="size-8 p-0"
                      aria-label="Edit folder"
                      title="Edit folder"
                      disabled={!folder.owner && !membersLoaded}
                      onclick={() => (editing = true)}
                    >
                      <Pencil size={16} />
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      class="mt-auto size-8 p-0"
                      aria-label="Delete folder"
                      title="Delete folder"
                      onclick={() => (deleteFolder = folder)}
                    >
                      <Trash size={16} />
                    </Button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<ConfirmDialog
  open={deleteFolder !== null}
  title="Delete folder"
  description={deleteFolder?.owner
    ? `Delete ${deleteFolder.name}? This removes that user's private folder and everything in it.`
    : `Delete ${deleteFolder?.name}? This removes the folder and everything in it.`}
  busy={deleting}
  onCancel={() => {
    if (!deleting) deleteFolder = null;
  }}
  onConfirm={confirmDelete}
/>
