<script lang="ts">
  import { Pencil, Trash } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ApiError } from "../../api";
  import { formatWhen } from "../../format";
  import type { UserData, UserViewFull } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Notice from "../ui/notice/notice.svelte";
  import ConfirmDialog from "../ui/confirm-dialog/confirm-dialog.svelte";
  import type { AdminContext } from "./categories";
  import UserForm from "../user/UserForm.svelte";

  let { context }: { context: AdminContext } = $props();

  let users = $state<UserViewFull[]>([]);

  let selectedId = $state<string | null>(null);
  let editing = $state(false);
  let creating = $state(false);

  let error = $state("");
  let notice = $state("");
  let deleteUser = $state<UserViewFull | null>(null);
  let deleting = $state(false);

  function blank() {
    selectedId = null;
    editing = false;
    creating = false;
    error = "";
    notice = "";
  }

  function toggleCreate() {
    const next = !creating;
    blank();
    creating = next;
  }

  function toggle(user: UserViewFull) {
    const next = selectedId === user.id ? null : user.id;
    blank();
    selectedId = next;
  }

  function startEdit() {
    error = "";
    notice = "";
    editing = true;
  }

  function stopEdit() {
    editing = false;
  }

  async function refresh() {
    users = await context.client.listUsers();
  }

  onMount(() => {
    refresh().catch(() => {
      error = "Could not load users";
    });
  });

  function describe(cause: ApiError) {
    if (cause.status === 409) return "That username or email is already in use";
    if (cause.status === 403) return "That change is not allowed";
    if (cause.status === 422) return "Check the username, email, and password";
    return "Could not save the user";
  }

  async function createUser(data: UserData) {
    if (!data.username || data.username.length === 0) {
      throw "Username cannot be empty";
    }

    if (data.email && data.email.length === 0) {
      throw "Email cannot be empty";
    }

    if (!data.password || data.password.length === 0) {
      throw "Password cannot be empty";
    }

    try {
      await context.client.createUser({
        username: data.username.trim(),
        password: data.password,
        email: data.email?.trim(),
        role: data.role,
      });

      blank();
      notice = "User created.";
      await refresh();
    } catch (e) {
      throw e instanceof ApiError
        ? describe(e)
        : "The server could not be reached";
    }
  }

  async function updateUser(data: UserData, userId: string) {
    if (data.username && data.username.length === 0) {
      throw "Username cannot be empty";
    }

    if (data.email && data.email.length === 0) {
      throw "Email cannot be empty";
    }

    if (data.password && data.password.length === 0) {
      data.password = undefined;
    }

    try {
      const updated = await context.client.updateUser(userId, {
        username: data.username?.trim(),
        email: data.email,
        role: data.role,
        password: data.password,
      });

      context.onAccountChanged(updated);

      editing = false;
      notice = "User updated.";

      await refresh();
    } catch (e) {
      throw e instanceof ApiError
        ? describe(e)
        : "The server could not be reached";
    }
  }

  function askDelete(user: UserViewFull) {
    error = "";
    deleteUser = user;
  }

  async function remove(userId: string) {
    try {
      await context.client.deleteUser(userId);
      deleteUser = null;
      blank();
      notice = "User removed.";
      await refresh();
      context.onAccountDeleted(userId);
    } catch (cause) {
      error =
        cause instanceof ApiError
          ? cause.status === 403
            ? "That user cannot be removed"
            : "Could not remove the user"
          : "The server could not be reached";
    }
  }

  async function confirmDelete() {
    if (!deleteUser || deleting) return;
    deleting = true;
    await remove(deleteUser.id);
    deleting = false;
  }
</script>

<div class="space-y-3">
  <div
    class="flex items-center justify-between gap-3 rounded-lg border-b px-3 py-1.5"
  >
    <h2 class="text-sm font-medium">Users</h2>
    <Button
      type="button"
      variant={creating ? "outline" : "ghost"}
      size="sm"
      class="size-9 p-0"
      aria-label={creating ? "Close new user form" : "Add user"}
      aria-expanded={creating}
      title="Add user"
      onclick={toggleCreate}
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
  </div>

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {/if}

  {#if notice}
    <Notice message={notice} />
  {/if}

  {#if creating}
    <div class="mx-auto max-w-lg">
      <UserForm
        {context}
        title="Add user"
        description="A private folder will be created with the account."
        save={(data) => createUser(data)}
        cancel={blank}
      />
    </div>
  {/if}

  {#if users.length === 0}
    <p class="text-sm text-muted-foreground">No users yet.</p>
  {:else}
    <ul class="divide-y overflow-hidden rounded-lg border">
      {#each users as user (user.id)}
        <li>
          <button
            type="button"
            class="flex min-h-11 w-full items-center justify-between gap-3 px-3 py-2 text-left {selectedId ===
            user.id
              ? 'bg-accent'
              : 'hover:bg-accent/60'}"
            aria-expanded={selectedId === user.id}
            onclick={() => toggle(user)}
          >
            <span class="min-w-0">
              <span class="block truncate text-sm font-medium"
                >{user.username}</span
              >
              <span
                class="block truncate text-xs text-muted-foreground capitalize"
              >
                {user.role}
              </span>
            </span>
            <svg
              viewBox="0 0 20 20"
              class="size-4 shrink-0 text-muted-foreground transition-transform {selectedId ===
              user.id
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

          <!-- USER INFO CARD -->

          {#if selectedId === user.id}
            <div class="border-t bg-muted/30 p-3">
              {#if editing}
                <div class="mx-auto max-w-lg">
                  <UserForm
                    {context}
                    {user}
                    title="Edit user"
                    description="Leave the password blank to keep the current one."
                    save={(data) => updateUser(data, user.id)}
                    remove={(userId) => remove(userId)}
                    cancel={stopEdit}
                  />
                </div>
              {:else}
                <div class="flex justify-between">
                  <div class="flex flex-wrap items-start justify-between gap-3">
                    <dl
                      class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm"
                    >
                      <dt class="text-muted-foreground">ID</dt>
                      <dd class="truncate font-mono text-xs leading-5">
                        {user.id}
                      </dd>
                      <dt class="text-muted-foreground">Username</dt>
                      <dd class="truncate">{user.username}</dd>
                      <dt class="text-muted-foreground">Email</dt>
                      <dd class="truncate">{user.email ?? "-"}</dd>
                      <dt class="text-muted-foreground">Role</dt>
                      <dd class="capitalize">{user.role}</dd>
                      <dt class="text-muted-foreground">Two-factor</dt>
                      <dd>{user.otpEnabled ? "Enabled" : "Disabled"}</dd>
                      <dt class="text-muted-foreground">Created</dt>
                      <dd>{formatWhen(user.createdAt)}</dd>
                      <dt class="text-muted-foreground">Updated</dt>
                      <dd>{formatWhen(user.updatedAt)}</dd>
                    </dl>
                  </div>
                  <div class="flex flex-col self-stretch">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      class="size-8 p-0"
                      aria-label="Edit user"
                      title="Edit user"
                      onclick={startEdit}
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
                      onclick={() => askDelete(user)}
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
  open={deleteUser !== null}
  title="Delete user"
  description={deleteUser
    ? deleteUser.id === context.currentUserId
      ? `Delete ${deleteUser.username}? You will be signed out.`
      : `Delete ${deleteUser.username}? This cannot be undone.`
    : ""}
  confirmLabel={deleteUser?.id === context.currentUserId
    ? "Delete and sign out"
    : "Delete"}
  busy={deleting}
  {error}
  onCancel={() => {
    if (!deleting) deleteUser = null;
  }}
  onConfirm={confirmDelete}
/>
