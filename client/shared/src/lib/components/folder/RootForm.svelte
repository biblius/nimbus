<script lang="ts">
  import { untrack } from "svelte";
  import { ApiError, ValidationError } from "../../api";
  import { formatQuota } from "../../format";
  import { toastError } from "../../toast";
  import type { Member, Root, RootData, UserViewFull } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";

  let {
    title,
    description,
    folder,
    users,
    members = [],
    save,
    cancel,
  }: {
    title: string;
    description: string;
    folder?: Root;
    users: UserViewFull[];
    members?: Member[];
    save: (root: RootData) => Promise<void>;
    cancel?: () => void;
  } = $props();

  let name = $state(untrack(() => folder?.name ?? ""));
  let quota = $state(untrack(() => (folder ? String(folder.quota) : "0")));
  let selected = $state(untrack(() => members.map((member) => member.id)));
  let quotaError = $state("");
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  const privateFolder = $derived(folder?.primary ?? false);

  const quotaPreview = $derived.by(() => {
    if (!/^\d+$/.test(quota.trim())) return "";
    return formatQuota(Number(quota));
  });

  function locked(id: string) {
    return members.some((member) => member.id === id && member.primary);
  }

  function toggle(id: string) {
    if (locked(id)) return;
    selected = selected.includes(id)
      ? selected.filter((memberId) => memberId !== id)
      : [...selected, id];
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
        name: name.trim(),
        quotaBytes: Number(quota),
        members: privateFolder
          ? members
              .filter((member) => member.primary)
              .map((member) => member.id)
          : selected,
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

  {#if !privateFolder}
    <fieldset class="space-y-2">
      <legend class="text-sm leading-none font-medium">Members</legend>
      {#if users.length === 0}
        <p class="text-sm text-muted-foreground">No users to assign.</p>
      {:else}
        <ul class="max-h-48 space-y-1 overflow-y-auto rounded-md border p-2">
          {#each users as user (user.id)}
            <li>
              <label class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={selected.includes(user.id)}
                  disabled={locked(user.id)}
                  onchange={() => toggle(user.id)}
                />
                <span class="truncate">{user.username}</span>
                {#if locked(user.id)}
                  <span class="text-xs text-muted-foreground">Owner</span>
                {/if}
              </label>
            </li>
          {/each}
        </ul>
      {/if}
    </fieldset>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Button type="submit" disabled={busy}>Confirm</Button>
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
  </div>
</form>
