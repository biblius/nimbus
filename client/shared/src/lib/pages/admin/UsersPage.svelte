<script lang="ts">
  import { Pencil, Trash, UserRound } from "@lucide/svelte";
  import { ApiError } from "../../api";
  import type { UserData, UserView, UserViewFull } from "../../types";
  import Button from "../../components/ui/button/button.svelte";
  import Chevron from "../../components/ui/chevron/chevron.svelte";
  import PaginatedList from "../../components/ui/paginated-list/paginated-list.svelte";
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

  let selectedId = $state<string | null>(null);
  let editing = $state(false);
  let creating = $state(false);
  let deleteUser = $state<UserViewFull | null>(null);
  let deleting = $state(false);
  let listRevision = $state(0);

  function reset() {
    selectedId = null;
    editing = false;
    creating = false;
  }

  function reloadList() {
    listRevision += 1;
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
    reloadList();
  }

  async function updateUser(data: UserData, userId: string) {
    const updated = await session.client.updateUser(userId, {
      username: data.username?.trim(),
      email: data.email || undefined,
      role: data.role,
      password: data.password || undefined,
    });

    if (session.user!.id === updated.id) {
      session.user = { ...session.user!, ...updated };
    }

    editing = false;

    reloadList();
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
      reloadList();

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

  const selected = (user: UserView) => selectedId === user.id;
</script>

<div class="space-y-5">
  <div class="flex flex-wrap items-end justify-between gap-3 border-b pb-4">
    <div class="min-w-0 space-y-1">
      <h2 class="text-lg font-semibold tracking-tight">Users</h2>
      <p class="text-sm text-muted-foreground">
        Accounts, roles, and private folder ownership.
      </p>
    </div>
    <Button
      type="button"
      variant={creating ? "outline" : "default"}
      size="sm"
      aria-expanded={creating}
      onclick={() => {
        const next = !creating;
        reset();
        creating = next;
      }}
    >
      {creating ? "Cancel" : "Add user"}
    </Button>
  </div>

  {#if creating}
    <div class="rounded-xl border bg-card p-4 sm:p-5">
      <div class="mx-auto max-w-lg">
        <UserForm
          title="Add user"
          description="A private folder will be created with the account."
          save={createUser}
          cancel={reset}
        />
      </div>
    </div>
  {/if}

  <PaginatedList
    load={(query) => session.client.listUsers(query)}
    {searchColumns}
    {sortColumns}
    {onLoaded}
    revision={listRevision}
  >
    {#snippet children(users)}
      {#if users.length === 0}
        <div
          class="flex flex-col items-center justify-center gap-2 px-6 py-16 text-center"
        >
          <UserRound class="size-9 text-muted-foreground opacity-50" />
          <p class="text-sm font-medium">No users match</p>
          <p class="max-w-sm text-sm text-muted-foreground">
            Try another search, or add a user to get started.
          </p>
        </div>
      {:else}
        <ul>
          {#each users as user, index (user.id)}
            <li class={index > 0 ? "border-t" : ""}>
              <button
                type="button"
                class="flex min-h-14 w-full items-center gap-3 px-3 py-2.5 text-left transition-colors {selected(
                  user,
                )
                  ? 'bg-accent'
                  : 'hover:bg-accent/50'}"
                aria-expanded={selected(user)}
                onclick={() => {
                  const next = selected(user) ? null : user.id;
                  reset();
                  selectedId = next;
                }}
              >
                <span class="min-w-0 flex-1">
                  <span class="flex min-w-0 items-center gap-2">
                    <span class="truncate text-sm font-medium"
                      >{user.username}</span
                    >
                    <span
                      class="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-medium capitalize {user.role ===
                      'admin'
                        ? 'bg-primary/15 text-primary'
                        : 'bg-muted text-muted-foreground'}"
                    >
                      {user.role}
                    </span>
                  </span>
                  <span
                    class="mt-0.5 block truncate text-xs text-muted-foreground"
                  >
                    {user.email ?? "No email"}
                  </span>
                </span>
                <Chevron expanded={selected(user)} />
              </button>

              {#if selected(user)}
                <div class="border-t bg-muted/25 px-3 py-4 sm:px-4">
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
                    <div
                      class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
                    >
                      <div class="min-w-0 flex-1">
                        <UserInfo {user} />
                      </div>
                      <div class="flex shrink-0 gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          aria-label="Edit user"
                          title="Edit user"
                          onclick={() => (editing = true)}
                        >
                          <Pencil size={15} />
                          Edit
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          aria-label="Delete user"
                          title="Delete user"
                          onclick={() => (deleteUser = user)}
                        >
                          <Trash size={15} />
                          Delete
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
