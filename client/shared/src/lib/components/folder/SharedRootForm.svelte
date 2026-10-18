<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ApiError, ValidationError } from "../../api";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Root, RootData, UserView } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Checkbox from "../ui/checkbox/checkbox.svelte";
  import Field from "../ui/field/field.svelte";
  import InfiniteList from "../ui/infinite-list/infinite-list.svelte";
  import QuotaField from "./QuotaField.svelte";

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

  const searchColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
  ];

  let membersReady = $state(untrack(() => folder == null));
  let membersFailed = $state(false);

  let name = $state(untrack(() => folder?.name ?? ""));
  let quotaBytes = $state(untrack(() => folder?.quota ?? 0));
  let selected = $state<string[]>([]);
  let quotaError = $state("");
  let quotaValid = $state(true);
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  const membersError = $derived(validation?.field("members"));
  const creating = $derived(folder == null);

  onMount(() => {
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

    validation = undefined;

    if (!quotaValid) {
      return;
    }

    if (creating && selected.length < 1) {
      return;
    }

    busy = true;

    try {
      await save({
        name,
        quotaBytes,
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

<form class="overflow-hidden rounded-xl border bg-card" onsubmit={onSubmit}>
  <div class="border-b px-5 py-4">
    <h3 class="text-sm font-semibold tracking-tight">{title}</h3>
    <p class="mt-1 text-sm text-muted-foreground">{description}</p>
  </div>

  <div class="space-y-4 px-5 py-4">
    <Field
      label="Name"
      id="folder-name"
      bind:value={name}
      error={validation?.field("name")}
      required
    />

    <QuotaField
      bind:bytes={quotaBytes}
      bind:error={quotaError}
      bind:valid={quotaValid}
      serverError={validation?.field("quota_bytes")}
    />

    <div class="space-y-2">
      <div class="flex items-baseline justify-between gap-3">
        <span class="text-sm leading-none font-medium">Members</span>
        <span class="text-xs text-muted-foreground">
          {selected.length} selected
        </span>
      </div>
      {#if membersError}
        <p class="text-xs text-destructive">
          {membersError.message || membersError.code}
        </p>
      {/if}
      {#if !membersReady && !membersFailed}
        <p class="text-sm text-muted-foreground">Loading…</p>
      {:else}
        {#if membersFailed}
          <p class="text-sm text-muted-foreground">Could not load members.</p>
        {/if}
        <InfiniteList
          columns={searchColumns}
          errorMessage="Could not load users."
          load={(query) =>
            session.client.listUsers({
              ...query,
              sort: { column: "username", direction: "asc" },
            })}
        >
          {#snippet children(users: UserView[])}
            {#if users.length === 0}
              <p class="px-3 py-6 text-center text-sm text-muted-foreground">
                No users to assign.
              </p>
            {:else}
              <ul class="divide-y">
                {#each users as user (user.id)}
                  <li>
                    <label
                      class="flex cursor-pointer items-center gap-3 px-3 py-2.5 text-sm hover:bg-accent"
                    >
                      <Checkbox
                        checked={selected.includes(user.id)}
                        onchange={() => toggle(user.id)}
                      />
                      <span class="min-w-0">
                        <span class="block truncate">{user.username}</span>
                        {#if user.email}
                          <span
                            class="block truncate text-xs text-muted-foreground"
                          >
                            {user.email}
                          </span>
                        {/if}
                      </span>
                    </label>
                  </li>
                {/each}
              </ul>
            {/if}
          {/snippet}
        </InfiniteList>
      {/if}
    </div>
  </div>

  <div
    class="flex items-center justify-end gap-2 border-t bg-muted/30 px-5 py-3"
  >
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
    <Button
      type="submit"
      disabled={busy || !membersReady || (creating && selected.length < 1)}
    >
      Save
    </Button>
  </div>
</form>
