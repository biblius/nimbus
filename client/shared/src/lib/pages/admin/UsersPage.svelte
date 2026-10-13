<script lang="ts">
  import { Pencil, Trash } from "@lucide/svelte";
  import { ApiError } from "../../api";
  import type { UserData, UserView, UserViewFull } from "../../types";
  import Button from "../../components/ui/button/button.svelte";
  import Chevron from "../../components/ui/chevron/chevron.svelte";
  import PaginatedList from "../../components/ui/paginated-list/paginated-list.svelte";
  import Plus from "../../components/ui/plus/plus.svelte";
  import ConfirmDialog from "../../components/ui/confirm-dialog/confirm-dialog.svelte";
  import UserForm from "../../components/user/UserForm.svelte";
  import { trackDeletion } from "../../deletion.svelte";
  import { logout, session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import UserInfo from "../../components/user/UserInfo.svelte";

  const searchColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
  ];

  const sortColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
    { value: "createdAt", label: "Created" },
    { value: "updatedAt", label: "Updated" },
  ];

  /**
   * The currently selected user.
   */
  let selectedId = $state<string | null>(null);

  /**
   * Whether a user is currently being edited.
   */
  let editing = $state(false);

  /**
   * Whether a user is currently being created.
   */
  let creating = $state(false);

  /**
   * Gets set when the delete icon is clicked to
   * prompt for confirmation before actually deleting.
   */
  let deleteUser = $state<UserViewFull | null>(null);

  /**
   * Gets set after deletion submition and reset back when
   * the request finishes.
   */
  let deleting = $state(false);

  function reset() {
    selectedId = null;
    editing = false;
    creating = false;
  }

  function onLoaded(listed: UserViewFull[]) {
    if (selectedId && !listed.some((user) => user.id === selectedId)) {
      reset();
    }
  }

  async function createUser(data: UserData) {
    if (!data.email) {
      data.email = undefined;
    }

    await session.client.createUser({
      username: data.username?.trim() ?? "",
      password: data.password ?? "",
      email: data.email,
      role: data.role,
    });

    reset();
  }

  async function updateUser(data: UserData, userId: string) {
    const updated = await session.client.updateUser(userId, {
      username: data.username?.trim(),
      email: data.email || undefined,
      role: data.role,
      password: data.password || undefined,
    });

    if (session.user?.id === updated.id) {
      session.user = updated;
    }

    editing = false;
  }

  async function confirmDelete() {
    if (!deleteUser || deleting) {
      return;
    }

    const userId = deleteUser.id;

    deleting = true;

    try {
      await trackDeletion(session.client.deleteUser(userId));

      deleteUser = null;

      reset();

      if (session.user?.id === userId) {
        await logout();
        return;
      }
    } catch (cause) {
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ApiError && cause.status === 403
            ? cause.message
            : "Could not remove user",
      );
    }
    deleting = false;
  }

  const selected = (user: UserView) => {
    return selectedId === user.id;
  };
</script>

<div class="space-y-3">
  <div class="flex h-12 items-center justify-between gap-3 border-b px-1">
    <h2 class="text-sm font-medium">Users</h2>
    <Button
      type="button"
      variant={creating ? "outline" : "ghost"}
      size="sm"
      class="size-9 p-0"
      aria-label={creating ? "Close new user form" : "Add user"}
      aria-expanded={creating}
      title="Add user"
      onclick={() => {
        const next = !creating;
        reset();
        creating = next;
      }}
    >
      <Plus bind:rotated={creating} />
    </Button>
  </div>

  {#if creating}
    <div class="mx-auto max-w-lg">
      <UserForm
        title="Add user"
        description="A private folder will be created with the account."
        save={createUser}
        cancel={reset}
      />
    </div>
  {/if}

  <PaginatedList
    load={(query) => session.client.listUsers(query)}
    {searchColumns}
    {sortColumns}
    {onLoaded}
  >
    {#snippet children(users)}
      {#if users.length === 0}
        <p class="text-sm text-muted-foreground">No users yet.</p>
      {:else}
        <ul class="divide-y overflow-hidden rounded-lg border">
          {#each users as user (user.id)}
            <li>
              <button
                type="button"
                class="flex min-h-11 w-full items-center justify-between gap-3 px-3 py-2 text-left {selected(
                  user,
                )
                  ? 'bg-accent'
                  : 'hover:bg-accent/60'}"
                aria-expanded={selected(user)}
                onclick={() => {
                  const next = selected(user) ? null : user.id;
                  reset();
                  selectedId = next;
                }}
              >
                <span class="min-w-0">
                  <span class="block truncate text-sm font-medium"
                    >{user.username}</span
                  >
                  <span class="block truncate text-xs text-muted-foreground">
                    {#if user.email}
                      {user.email} ·
                    {/if}
                    <span class="capitalize">{user.role}</span>
                  </span>
                </span>
                <Chevron expanded={selected(user)} />
              </button>

              {#if selected(user)}
                <div class="border-t bg-muted/30 p-3">
                  {#if editing}
                    <div class="mx-auto max-w-lg">
                      <UserForm
                        {user}
                        title="Edit user"
                        description="Leave the password blank to keep the current one."
                        save={(data) => updateUser(data, user.id)}
                        cancel={() => (editing = false)}
                      />
                    </div>
                  {:else}
                    <div class="flex justify-between gap-3">
                      <div class="min-w-0 flex-1">
                        <UserInfo {user} />
                      </div>
                      <div class="flex flex-col self-stretch">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          class="size-8 p-0"
                          aria-label="Edit user"
                          title="Edit user"
                          onclick={() => (editing = true)}
                        >
                          <Pencil size={16} />
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          class="mt-auto size-8 p-0"
                          aria-label="Delete user"
                          title="Delete user"
                          onclick={() => (deleteUser = user)}
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
    {/snippet}
  </PaginatedList>
</div>

<ConfirmDialog
  open={deleteUser !== null && !deleting}
  title="Delete user"
  description={deleteUser == null
    ? ""
    : deleteUser.id === session.user?.id
      ? `Delete ${deleteUser.username}? You will be signed out.`
      : `Delete ${deleteUser.username}? This cannot be undone.`}
  confirmLabel={deleteUser?.id === session.user?.id
    ? "Delete and sign out"
    : "Delete"}
  busy={deleting}
  onCancel={() => {
    if (!deleting) {
      deleteUser = null;
    }
  }}
  onConfirm={confirmDelete}
/>
