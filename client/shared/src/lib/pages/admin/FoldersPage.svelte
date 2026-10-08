<script lang="ts">
  import { onMount } from "svelte";
  import { ValidationError } from "../../api";
  import { formatQuota, formatDatetime } from "../../format";
  import { toastError } from "../../toast";
  import type { Folder } from "../../types";
  import Button from "../../components/ui/button/button.svelte";
  import Field from "../../components/ui/field/field.svelte";
  import { route } from "../../router";
  import { session } from "../../session.svelte";

  let folders = $state<Folder[]>([]);
  let selectedId = $state<string | null>(null);
  let name = $state("");
  let quota = $state("0");
  let quotaError = $state("");
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  const selected = $derived(
    folders.find((folder) => folder.id === selectedId) ?? null,
  );
  const quotaPreview = $derived.by(() => {
    if (!/^\d+$/.test(quota.trim())) return "";
    return formatQuota(Number(quota));
  });

  function choose(folder: Folder) {
    selectedId = folder.id;
    name = folder.name;
    quota = String(folder.quota);
    quotaError = "";
    validation = undefined;
  }

  onMount(() => {
    session.client
      .listFolders()
      .then((next) => {
        folders = next;
        // "Edit root" in the file browser opens `/admin/folders?id=<root>`.
        const requested =
          next.find((folder) => folder.id === route.search.id) ?? next[0];
        if (requested) choose(requested);
      })
      .catch((cause) => {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load folders",
        );
      });
  });

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const folder = selected;
    if (!folder) return;
    quotaError = "";
    validation = undefined;
    if (!/^\d+$/.test(quota.trim())) {
      quotaError = "Quota must be a whole number of bytes, or 0 for unlimited";
      return;
    }
    const quotaBytes = Number(quota);
    if (!Number.isSafeInteger(quotaBytes)) {
      quotaError = "Quota must be a whole number of bytes, or 0 for unlimited";
      return;
    }

    busy = true;
    try {
      const updated = await session.client.updateFolder(
        folder.id,
        name.trim(),
        quotaBytes,
      );
      folders = folders.map((item) =>
        item.id === updated.id ? updated : item,
      );
      name = updated.name;
      quota = String(updated.quota);
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : "Could not update the folder",
      );
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="grid items-start gap-6 lg:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]"
>
  <div class="space-y-3">
    <h2 class="text-lg font-medium">Folders</h2>
    {#if folders.length === 0}
      <p class="text-sm text-muted-foreground">No folders yet.</p>
    {:else}
      <ul class="divide-y overflow-hidden rounded-lg border">
        {#each folders as folder (folder.id)}
          <li>
            <button
              type="button"
              class="flex min-h-11 w-full flex-col items-start justify-center px-3 py-2 text-left {selectedId ===
              folder.id
                ? 'bg-accent'
                : 'hover:bg-accent/60'}"
              onclick={() => choose(folder)}
            >
              <span class="w-full truncate text-sm font-medium"
                >{folder.name}</span
              >
              <span class="text-xs text-muted-foreground"
                >{formatQuota(folder.quota)}</span
              >
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if selected}
    <form
      class="max-w-lg space-y-4 rounded-xl border bg-card p-4"
      onsubmit={save}
    >
      <div>
        <h3 class="text-base font-medium">Edit folder</h3>
        <p class="text-sm text-muted-foreground">
          Updated {formatDatetime(selected.updatedAt)}. A quota of 0 means
          unlimited.
        </p>
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
      <Button type="submit" disabled={busy}>Save</Button>
    </form>
  {:else}
    <p class="text-sm text-muted-foreground">
      Select a folder to edit its name and quota.
    </p>
  {/if}
</div>
