<script lang="ts">
  import { Check, Pencil, X } from "@lucide/svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { FileEntry } from "../../types";
  import CodeMirrorEditor from "../CodeMirrorEditor.svelte";

  let {
    rootId,
    fileId,
    name,
    onUpdated,
    editing = $bindable(false),
  }: {
    rootId: string;
    fileId: string;
    name: string;
    onUpdated?: (entry: FileEntry) => void;
    editing?: boolean;
  } = $props();

  let text = $state("");
  let draft = $state("");
  let saving = $state(false);
  let loading = $state(true);
  let error = $state("");

  function startEdit() {
    draft = text;
    editing = true;
  }

  function cancelEdit() {
    draft = text;
    editing = false;
  }

  async function save() {
    if (saving) {
      return;
    }

    saving = true;

    try {
      const entry = await session.client.overwrite(
        rootId,
        fileId,
        new Blob([draft], { type: "text/plain" }),
        name,
      );

      text = draft;
      editing = false;
      onUpdated?.(entry);
    } catch {
      toastError("Could not save file");
    } finally {
      saving = false;
    }
  }

  export function cancelIfEditing(): boolean {
    if (!editing) {
      return false;
    }
    cancelEdit();
    return true;
  }

  $effect(() => {
    const id = fileId;

    text = "";
    draft = "";
    editing = false;
    error = "";
    loading = true;

    let cancelled = false;

    session.client
      .content(rootId, id)
      .then(async (blob) => {
        const value = await blob.text();
        if (cancelled) {
          return;
        }
        text = value;
        draft = value;
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = "Could not load file";
        loading = false;
      });

    return () => {
      cancelled = true;
    };
  });
</script>

<div class="flex h-full w-full flex-col gap-3">
  {#if !loading && !error}
    <div class="flex shrink-0 justify-end gap-2">
      {#if editing}
        <button
          type="button"
          class="flex size-10 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18 disabled:opacity-50"
          aria-label="Save"
          disabled={saving}
          onclick={save}
        >
          <Check size={18} strokeWidth={2} />
        </button>
        <button
          type="button"
          class="flex size-10 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18 disabled:opacity-50"
          aria-label="Cancel"
          disabled={saving}
          onclick={cancelEdit}
        >
          <X size={18} strokeWidth={2} />
        </button>
      {:else}
        <button
          type="button"
          class="flex size-10 items-center justify-center rounded-full bg-white/10 text-white ring-1 ring-white/15 backdrop-blur-md transition-colors hover:bg-white/18"
          aria-label="Edit"
          onclick={startEdit}
        >
          <Pencil size={18} strokeWidth={2} />
        </button>
      {/if}
    </div>
  {/if}

  {#if loading}
    <div
      class="m-auto size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
      aria-label="Loading"
    ></div>
  {:else if error}
    <p class="m-auto text-sm text-red-300">{error}</p>
  {:else}
    <CodeMirrorEditor
      bind:value={draft}
      editable={editing}
      aria-label={editing ? `Edit ${name}` : name}
    />
  {/if}
</div>
