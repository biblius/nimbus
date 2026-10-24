<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ApiError, ValidationError } from "../../api-errors";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import { AccessMode, type Root, type RootData, type UserView } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Checkbox from "../ui/checkbox/checkbox.svelte";
  import Field from "../ui/field/field.svelte";
  import InfiniteList from "../ui/infinite-list/infinite-list.svelte";
  import AccessPills from "./AccessPills.svelte";
  import MemberAvatar from "./MemberAvatar.svelte";
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
  let selected = $state<Record<string, number>>({});
  let quotaError = $state("");
  let quotaValid = $state(true);
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  const membersError = $derived(validation?.field("members"));
  const creating = $derived(folder == null);
  const selectedCount = $derived(Object.keys(selected).length);

  onMount(() => {
    if (!folder) {
      return;
    }

    const id = folder.id;

    session.client
      .listAllMembers(id)
      .then((members) => {
        for (const member of members) {
          selected[member.id] = member.accessMode;
        }
        membersReady = true;
      })
      .catch((cause) => {
        membersFailed = true;
        toastError(loadErrorMessage(cause));
      });
  });

  function loadErrorMessage(cause: unknown): string {
    if (cause instanceof TypeError) {
      return "The server could not be reached";
    }
    return "Could not load members";
  }

  function saveErrorMessage(cause: unknown): string {
    if (cause instanceof TypeError) {
      return "The server could not be reached";
    }
    if (cause instanceof ApiError && cause.status === 403) {
      return cause.message;
    }
    return "Could not save the folder";
  }

  function toggleMembership(id: string) {
    if (id in selected) {
      delete selected[id];
    } else {
      selected[id] = AccessMode.DEFAULT;
    }
  }

  function toggleFlag(id: string, flag: number) {
    const mode = selected[id];
    if (mode === undefined) {
      return;
    }
    if ((mode & flag) !== 0) {
      selected[id] = mode & ~flag;
    } else {
      selected[id] = mode | flag;
    }
  }

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    validation = undefined;

    if (!quotaValid) {
      return;
    }

    if (creating && selectedCount < 1) {
      return;
    }

    busy = true;

    try {
      await save({
        name,
        quotaBytes,
        members: Object.entries(selected).map(([userId, accessMode]) => ({
          userId,
          accessMode,
        })),
      });
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(saveErrorMessage(cause));
    } finally {
      busy = false;
    }
  }
</script>

<form class="space-y-5" onsubmit={onSubmit}>
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
      <Button
        type="submit"
        size="sm"
        disabled={busy || !membersReady || (creating && selectedCount < 1)}
      >
        Save
      </Button>
    </div>
  </div>

  <section class="space-y-3">
    <p
      class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
    >
      Details
    </p>

    <Field
      label="Name"
      id="folder-name"
      dense
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
  </section>

  <section class="space-y-3 border-t pt-4">
    <div class="flex items-center gap-2">
      <p
        class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
      >
        People with access
      </p>
      <span
        class="inline-flex min-w-5 items-center justify-center rounded-full bg-primary/15 px-1.5 py-0.5 text-[11px] font-semibold tabular-nums text-primary"
      >
        {selectedCount}
      </span>
    </div>

    {#if membersError}
      <p class="text-xs text-destructive">
        {membersError.message || membersError.code}
      </p>
    {/if}
    {#if creating && selectedCount < 1}
      <p class="text-xs text-muted-foreground">
        Select at least one person to share this folder with.
      </p>
    {/if}

    {#if !membersReady && !membersFailed}
      <p class="text-sm text-muted-foreground">Loading…</p>
    {:else}
      {#if membersFailed}
        <p class="text-sm text-muted-foreground">Could not load members.</p>
      {/if}
      <div class="overflow-hidden rounded-lg border bg-card shadow-sm">
        <InfiniteList
          class="space-y-0"
          searchClass="border-b bg-muted/25 p-2"
          scrollerClass="max-h-80 rounded-none border-0"
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
              <p class="px-4 py-8 text-center text-sm text-muted-foreground">
                No users to assign.
              </p>
            {:else}
              <ul class="divide-y divide-border/60">
                {#each users as user (user.id)}
                  {@const mode = selected[user.id]}
                  {@const isSelected = mode !== undefined}
                  <li
                    class={[
                      "transition-colors",
                      isSelected
                        ? "bg-primary/[0.06]"
                        : "hover:bg-accent/40",
                    ]}
                  >
                    <div
                      class="flex flex-col gap-2 px-3 py-2.5 sm:flex-row sm:items-center sm:gap-3"
                    >
                      <label
                        class="flex min-w-0 flex-1 cursor-pointer items-center gap-3"
                      >
                        <Checkbox
                          checked={isSelected}
                          onchange={() => toggleMembership(user.id)}
                          aria-label={`Share with ${user.username}`}
                        />
                        <MemberAvatar name={user.username} seed={user.id} />
                        <span class="min-w-0">
                          <span class="block truncate text-sm font-medium">
                            {user.username}
                          </span>
                          {#if user.email}
                            <span
                              class="block truncate text-xs text-muted-foreground"
                            >
                              {user.email}
                            </span>
                          {/if}
                        </span>
                      </label>
                      {#if isSelected}
                        <div class="flex shrink-0 pl-8 sm:pl-0">
                          <AccessPills
                            accessMode={mode}
                            interactive
                            onToggle={(flag) => toggleFlag(user.id, flag)}
                          />
                        </div>
                      {/if}
                    </div>
                  </li>
                {/each}
              </ul>
            {/if}
          {/snippet}
        </InfiniteList>
      </div>
    {/if}
  </section>
</form>
