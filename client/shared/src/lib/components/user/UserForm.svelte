<script lang="ts">
  import { ApiError, ValidationError, type ValidationField } from "../../api";
  import { toastError } from "../../toast";
  import type { Role, UserData, UserView } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";

  let {
    title,
    description,
    user = $bindable(),
    /** When true, role is hidden and kept as the existing value. */
    selfService = false,
    save,
    cancel,
  }: {
    title: string;
    description: string;
    user?: UserView;
    selfService?: boolean;
    save: (user: UserData) => Promise<void>;
    cancel?: () => void;
  } = $props();

  let username = $state(user?.username ?? "");
  let email = $state(user?.email ?? "");
  let role = $state<Role>(user?.role ?? "user");
  let password = $state("");
  let passwordConfirm = $state("");

  let busy = $state(false);
  let validation = $state<ValidationError | undefined>();
  let confirmError = $state<ValidationField | undefined>();

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    busy = true;
    validation = undefined;
    confirmError = undefined;

    if (selfService && password && password !== passwordConfirm) {
      confirmError = {
        field: "passwordConfirm",
        code: "mismatch",
        params: {},
        message: "Passwords do not match",
        location: "/passwordConfirm",
      };
      busy = false;
      return;
    }

    try {
      await save({
        username,
        email,
        role: selfService ? (user?.role ?? role) : role,
        password,
      });
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

<form class="space-y-3" onsubmit={onSubmit}>
  <div class="flex items-end justify-between gap-3 border-b pb-2.5">
    <div class="min-w-0">
      <h3 class="text-sm font-semibold tracking-tight">{title}</h3>
      <p class="mt-0.5 text-xs text-muted-foreground">{description}</p>
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
      {#if cancel}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={busy}
          onclick={cancel}
        >
          Cancel
        </Button>
      {/if}
      <Button type="submit" size="sm" disabled={busy}>Save</Button>
    </div>
  </div>

  <div class="grid gap-2.5 sm:grid-cols-2">
    <Field
      label="Username"
      id="account-username"
      dense
      bind:value={username}
      error={validation?.field("username")}
      autocomplete="off"
      required
    />

    <Field
      label="Email"
      id="account-email"
      type="email"
      dense
      bind:value={email}
      error={validation?.field("email")}
      autocomplete="off"
    />

    {#if !selfService}
      <Field label="Role" id="account-role" dense>
        {#snippet children()}
          <select
            id="account-role"
            class="flex h-8 w-full rounded-md border border-input bg-field px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            bind:value={role}
          >
            <option value="user">User</option>
            <option value="admin">Admin</option>
          </select>
        {/snippet}
      </Field>
    {/if}

    <div class="sm:col-span-2 space-y-2.5">
      <Field
        label="Password"
        id="account-password"
        type="password"
        dense
        bind:value={password}
        error={validation?.field("password")}
        autocomplete="new-password"
        placeholder="Leave blank to keep"
      />

      {#if selfService && password}
        <Field
          label="Repeat password"
          id="account-password-confirm"
          type="password"
          dense
          bind:value={passwordConfirm}
          error={confirmError}
          autocomplete="new-password"
          placeholder="Repeat new password"
          required
        />
      {/if}
    </div>
  </div>
</form>
