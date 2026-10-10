<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ApiError, ValidationError } from "../../api";
  import { formatQuota } from "../../format";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData, UserViewFull } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";

  let {
    title,
    description,
    folder,
    save,
    cancel,
  }: {
    title: string;
    description: string;
    folder?: Root;
    save: (root: RootData) => Promise<void>;
    cancel?: () => void;
  } = $props();

  let users = $state<UserViewFull[]>([]);
  let usersLoaded = $state(false);
  let usersFailed = $state(false);
  let membersReady = $state(untrack(() => folder == null));
  let membersFailed = $state(false);

  let name = $state(untrack(() => folder?.name ?? ""));
  let quota = $state(untrack(() => (folder ? String(folder.quota) : "0")));
  let selected = $state<string[]>([]);
  let quotaError = $state("");
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  onMount(() => {
    session.client
      .listUsers()
      .then((people) => {
        users = people;
        usersLoaded = true;
      })
      .catch((cause) => {
        usersLoaded = true;
        usersFailed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load users",
        );
      });

    if (!folder) {
      return;
    }

    const id = folder.id;

    session.client
      .listAllMembers(id)
      .then((members) => {
        for (const member of members) {
          selected.push(member.id);
        }
        membersReady = true;
      })
      .catch((cause) => {
        membersFailed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load members",
        );
      });
  });

  const quotaPreview = $derived.by(() => {
    if (!/^\d+$/.test(quota.trim())) {
      return "";
    }
    return formatQuota(Number(quota));
  });

  function toggle(id: string) {
    const index = selected.indexOf(id);
    if (index === -1) {
      selected.push(id);
    } else {
      selected.splice(index, 1);
    }
  }

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    quotaError = "";
    validation = undefined;

    if (!/^\d+$/.test(quota.trim()) || !Number.isSafeInteger(Number(quota))) {
      quotaError = "Quota must be a whole number of bytes, or 0 for unlimited";
      return;
    }

    busy = true;

    try {
      await save({
        name,
        quotaBytes: Number(quota),
        members: selected,
      });
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ApiError && cause.status === 403
            ? cause.message
            : "Could not save the folder",
      );
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
    label="Name"
    id="folder-name"
    bind:value={name}
    error={validation?.field("name")}
    required
  />

  <Field
    label="Quota (bytes)"
    id="folder-quota"
    inputmode="numeric"
    bind:value={quota}
    error={validation?.field("quota_bytes")}
    class={quotaError
      ? "border-destructive focus-visible:ring-destructive"
      : ""}
    required
  />
  {#if quotaPreview}
    <p class="text-xs text-muted-foreground">{quotaPreview}</p>
  {/if}
  {#if quotaError}
    <p class="text-sm text-destructive">{quotaError}</p>
  {/if}

  <fieldset class="space-y-2">
    <legend class="text-sm leading-none font-medium">Members</legend>
    {#if !membersReady && !membersFailed}
      <p class="text-sm text-muted-foreground">Loading…</p>
    {:else if membersFailed}
      <p class="text-sm text-muted-foreground">Could not load members.</p>
    {:else if !usersLoaded}
      <p class="text-sm text-muted-foreground">Loading…</p>
    {:else if usersFailed}
      <p class="text-sm text-muted-foreground">Could not load users.</p>
    {:else if users.length === 0}
      <p class="text-sm text-muted-foreground">No users to assign.</p>
    {:else}
      <ul class="max-h-48 space-y-1 overflow-y-auto rounded-md border p-2">
        {#each users as user (user.id)}
          <li>
            <label class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={selected.includes(user.id)}
                onchange={() => toggle(user.id)}
              />
              <span class="truncate">{user.username}</span>
            </label>
          </li>
        {/each}
      </ul>
    {/if}
  </fieldset>

  <div class="flex flex-wrap items-center gap-2">
    <Button type="submit" disabled={busy || !membersReady}>Confirm</Button>
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
  </div>
</form>
