<script lang="ts">
  import { ApiError, ValidationError, type ValidationField } from "../../api";
  import { toastError } from "../../toast";
  import type { Role, UserData, UserView } from "../../types";
  import { Info } from "@lucide/svelte";
  import Banner from "../ui/banner/banner.svelte";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";
  import Select from "../ui/select/select.svelte";

  let {
    title = "",
    description = "",
    user = $bindable(),
    /** When true, role is hidden and kept as the existing value. */
    selfService = false,
    save,
    cancel,
  }: {
    title?: string;
    description?: string;
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

<form class="space-y-4" onsubmit={onSubmit}>
  {#if !selfService}
    <div class="flex items-end justify-between gap-3 border-b pb-3">
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
  {/if}

  <div class="space-y-3">
    {#if !selfService}
      <p
        class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
      >
        Account
      </p>
    {/if}

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
        <Select id="account-role" class="h-8" bind:value={role}>
          <option value="user">User</option>
          <option value="admin">Admin</option>
        </Select>
      </Field>
    {/if}
  </div>

  <div class="space-y-3 border-t pt-4">
    {#if !selfService}
      <p
        class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
      >
        Password
      </p>
    {/if}

    {#if selfService}
      <Banner tone="info">
        {#snippet icon()}<Info />{/snippet}
        Changing your password will sign out all other sessions.
      </Banner>
    {/if}

    <div class="space-y-3">
      <Field
        label="Password"
        id="account-password"
        type="password"
        dense
        bind:value={password}
        error={validation?.field("password")}
        autocomplete="new-password"
      />

      {#if selfService}
        <Field
          label="Confirm password"
          id="account-password-confirm"
          type="password"
          dense
          bind:value={passwordConfirm}
          error={confirmError}
          autocomplete="new-password"
          placeholder="Confirm new password"
          required={!!password}
        />
      {/if}
    </div>
  </div>

  {#if selfService}
    <div class="flex justify-end gap-1.5">
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
  {/if}
</form>
