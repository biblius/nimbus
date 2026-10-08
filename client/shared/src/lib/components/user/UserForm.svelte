<script lang="ts">
  import type { Role, UserData, UserView } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Notice from "../ui/notice/notice.svelte";
  import Input from "../ui/input/input.svelte";
  import Label from "../ui/label/label.svelte";

  let {
    title,
    description,
    user = $bindable(),
    save,
    cancel,
  }: {
    title: string;
    description: string;
    user?: UserView;
    save: (user: UserData) => Promise<void>;
    cancel?: () => void;
  } = $props();

  let username = $state(user?.username ?? "");
  let email = $state(user?.email ?? "");
  let role = $state<Role>(user?.role ?? "user");
  let password = $state("");

  let busy = $state(false);
  let error = $state("");
  let notice = $state("");

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = "";
    try {
      await save({ username, email, role, password });
      if (user) {
        password = "";
      } else {
        reset();
      }
    } catch (cause) {
      error = typeof cause === "string" ? cause : "Could not save the user";
    } finally {
      busy = false;
    }
  }

  function reset() {
    username = "";
    email = "";
    role = "user";
    password = "";
  }
</script>

<form
  class="max-w-lg space-y-4 rounded-xl border bg-card p-4"
  onsubmit={onSubmit}
>
  <div>
    <h3 class="text-base font-medium">
      {title}
      <!-- {editing ? "Edit user" : "Add user"} -->
    </h3>
    <p class="text-sm text-muted-foreground">
      {description}
      <!-- {editing
        ? "Leave the password blank to keep the current one."
        : "A private folder is created with the account."}
      -->
    </p>
  </div>

  <div class="space-y-2">
    <Label for="account-username">Username</Label>
    <Input
      id="account-username"
      bind:value={username}
      autocomplete="off"
      required
    />
  </div>

  <div class="space-y-2">
    <Label for="account-email">Email</Label>
    <Input
      id="account-email"
      type="email"
      bind:value={email}
      autocomplete="off"
    />
  </div>

  <div class="space-y-2">
    <Label for="account-role">Role</Label>
    <select
      id="account-role"
      class="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
      bind:value={role}
    >
      <option value="user">User</option>
      <option value="admin">Admin</option>
    </select>
  </div>

  <div class="space-y-2">
    <Label for="account-password">Password</Label>
    <Input
      id="account-password"
      type="password"
      bind:value={password}
      autocomplete="new-password"
    />
  </div>

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {/if}
  {#if notice}
    <Notice message={notice} />
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Button type="submit" disabled={busy}>Confirm</Button>
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
  </div>
</form>
