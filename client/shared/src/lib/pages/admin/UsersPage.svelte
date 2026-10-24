<script lang="ts">
  import { Pencil, Trash, UserRound } from "@lucide/svelte";
  import { ApiError } from "../../api-errors";
  import type { UserData, UserView } from "../../types";
  import Button from "../../components/ui/button/button.svelte";
  import PaginatedList from "../../components/ui/paginated-list/paginated-list.svelte";
  import ConfirmDialog from "../../components/ui/confirm-dialog/confirm-dialog.svelte";
  import SortHeader, {
    type SortState,
  } from "../../components/ui/sort-header/sort-header.svelte";
  import UserForm from "../../components/user/UserForm.svelte";
  import { trackDeletion } from "../../deletion.svelte";
  import { formatDatetime } from "../../format";
  import { logout, session } from "../../session.svelte";
  import { kv } from "../../storage";
  import { toastError } from "../../toast";
  import { cn } from "../../utils";

  type UserSortColumn = "username" | "email" | "createdAt";

  const searchColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
  ];

  const DEFAULT_SORT: SortState<UserSortColumn> = {
    column: "username",
    direction: "asc",
  };

  /** Matches FileBrowser: name grows; meta columns appear at wider container sizes. */
  const columns = cn(
    "grid grid-cols-[minmax(0,1fr)_5.5rem] items-center gap-x-3 px-3",
    "@md:grid-cols-[minmax(7rem,1.1fr)_minmax(0,1.25fr)_5.5rem]",
    "@xl:grid-cols-[minmax(7rem,1.1fr)_minmax(0,1.25fr)_4.75rem_4.5rem_minmax(0,10.5rem)_5.5rem]",
  );

  const storedSort = kv.usersSort.get();

  let sort = $state<SortState<UserSortColumn>>({
    column: storedSort.column as UserSortColumn,
    direction: storedSort.direction,
  });
  let creating = $state(false);
  let editingUser = $state<UserView | null>(null);
  let deleteUser = $state<UserView | null>(null);
  let deleting = $state(false);
  let listRevision = $state(0);

  function reset() {
    creating = false;
    editingUser = null;
  }

  function reloadList() {
    listRevision += 1;
  }

  function startCreate() {
    editingUser = null;
    creating = !creating;
  }

  function startEdit(user: UserView) {
    creating = false;
    editingUser = user;
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

    reset();
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
</script>

{#snippet sortHeader(
  column: UserSortColumn,
  label: string,
  cellClass?: string,
)}
  <SortHeader
    {label}
    {column}
    bind:sort
    defaultSort={DEFAULT_SORT}
    onCycle={() => kv.usersSort.set(sort)}
    class={cellClass}
  />
{/snippet}

<div class="min-h-0 flex-1 space-y-3 overflow-y-auto overscroll-contain">
  <div class="flex flex-wrap items-end justify-between gap-3 border-b pb-3">
    <div class="min-w-0 space-y-0.5">
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
      onclick={startCreate}
    >
      {creating ? "Cancel" : "Add user"}
    </Button>
  </div>

  {#if creating}
    <div class="rounded-lg border bg-card p-4 sm:p-5">
      <div class="mx-auto max-w-lg">
        <UserForm
          title="Add user"
          description="A private folder will be created with the account."
          save={createUser}
          cancel={reset}
        />
      </div>
    </div>
  {:else if editingUser}
    {@const editId = editingUser.id}
    {#key editId}
      <div class="rounded-lg border bg-card p-4 sm:p-5">
        <div class="mx-auto max-w-lg">
          <UserForm
            user={editingUser}
            title="Editing user {editingUser.username}"
            description="Leave the password blank to keep the current one."
            save={(data) => updateUser(data, editId)}
            cancel={reset}
          />
        </div>
      </div>
    {/key}
  {/if}

  <PaginatedList
    load={(query) => session.client.listUsers(query)}
    {searchColumns}
    {sort}
    revision={listRevision}
  >
    {#snippet listHeader()}
      <div class="{columns} h-9 text-xs font-medium text-muted-foreground">
        {@render sortHeader("username", "Username")}
        {@render sortHeader("email", "Email", "hidden @md:flex")}
        <span class="hidden @xl:block">Role</span>
        <span class="hidden @xl:block">2FA</span>
        {@render sortHeader("createdAt", "Created", "hidden @xl:flex")}
        <span class="sr-only">Actions</span>
      </div>
    {/snippet}

    {#snippet children(users)}
      {#if users.length === 0}
        <div
          class="flex flex-col items-center justify-center gap-1.5 px-6 py-10 text-center"
        >
          <UserRound class="size-8 text-muted-foreground opacity-50" />
          <p class="text-sm font-medium">No users match</p>
          <p class="max-w-sm text-sm text-muted-foreground">
            Try another search, or add a user to get started.
          </p>
        </div>
      {:else}
        <ul class="divide-y">
          {#each users as user (user.id)}
            <li class="{columns} min-h-11 py-1.5 hover:bg-accent/60">
              <div class="min-w-0">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="truncate text-sm font-medium"
                    >{user.username}</span
                  >
                  <span
                    class="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-medium capitalize @xl:hidden {user.role ===
                    'admin'
                      ? 'bg-primary/15 text-primary'
                      : 'bg-muted text-muted-foreground'}"
                  >
                    {user.role}
                  </span>
                </div>
                <p
                  class="mt-0.5 truncate text-xs text-muted-foreground @md:hidden"
                >
                  {user.email ?? "No email"}
                </p>
              </div>

              <span
                class="hidden min-w-0 truncate text-sm text-muted-foreground @md:block"
              >
                {user.email ?? "-"}
              </span>

              <span class="hidden @xl:block">
                <span
                  class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium capitalize {user.role ===
                  'admin'
                    ? 'bg-primary/15 text-primary'
                    : 'bg-muted text-muted-foreground'}"
                >
                  {user.role}
                </span>
              </span>

              <span class="hidden @xl:block">
                <span
                  class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium {user.otpEnabled
                    ? 'bg-success/15 text-success'
                    : 'bg-warning/15 text-warning'}"
                >
                  {user.otpEnabled ? "On" : "Off"}
                </span>
              </span>

              <span
                class="hidden min-w-0 truncate text-sm text-muted-foreground @xl:block"
              >
                {formatDatetime(user.createdAt)}
              </span>

              <div class="flex items-center justify-end gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Edit {user.username}"
                  title="Edit"
                  onclick={() => startEdit(user)}
                >
                  <Pencil size={16} />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                  aria-label="Delete {user.username}"
                  title="Delete"
                  onclick={() => (deleteUser = user)}
                >
                  <Trash size={16} />
                </Button>
              </div>
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
