<script lang="ts">
  import { ApiError, ValidationError } from "../../api";
  import { toastError } from "../../toast";
  import type { Role, UserData, UserView } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";

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
  let validation = $state<ValidationError | undefined>();

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    busy = true;
    validation = undefined;

    try {
      await save({ username, email, role, password });
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
      } else {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : cause instanceof ApiError && cause.status === 409
              ? "That username or email is already in use"
              : "Could not save the user",
        );
      }
    } finally {
      busy = false;
    }
  }
</script>

<form
  class="max-w-lg space-y-4 rounded-xl border bg-card p-4"
  onsubmit={onSubmit}
>
  <div>
    <h3 class="text-base font-medium">{title}</h3>
    <p class="text-sm text-muted-foreground">{description}</p>
  </div>

  <Field
    label="Username"
    id="account-username"
    bind:value={username}
    error={validation?.field("username")}
    autocomplete="off"
    required
  />

  <Field
    label="Email"
    id="account-email"
    type="email"
    bind:value={email}
    error={validation?.field("email")}
    autocomplete="off"
  />

  <Field label="Role" id="account-role">
    {#snippet children()}
      <select
        id="account-role"
        class="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        bind:value={role}
      >
        <option value="user">User</option>
        <option value="admin">Admin</option>
      </select>
    {/snippet}
  </Field>

  <Field
    label="Password"
    id="account-password"
    type="password"
    bind:value={password}
    error={validation?.field("password")}
    autocomplete="new-password"
  />

  <div class="flex flex-wrap items-center gap-2">
    <Button type="submit" disabled={busy}>Confirm</Button>
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
  </div>
</form>
