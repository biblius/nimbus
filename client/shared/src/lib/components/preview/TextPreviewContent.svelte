<script lang="ts">
  import { Check, Pencil, X } from "@lucide/svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { FileEntry } from "../../types";

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
  {:else if editing}
    <textarea
      class="min-h-0 w-full flex-1 resize-none rounded-xl bg-zinc-900/80 px-5 py-4 font-mono text-sm leading-relaxed text-zinc-100 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] ring-1 ring-white/10 outline-none focus:ring-white/25"
      bind:value={draft}
      spellcheck="false"
      aria-label="Edit {name}"
    ></textarea>
  {:else}
    <pre
      class="min-h-0 w-full flex-1 overflow-auto rounded-xl bg-zinc-900/80 px-5 py-4 font-mono text-sm leading-relaxed whitespace-pre-wrap text-zinc-100 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] ring-1 ring-white/10">{text}</pre>
  {/if}
</div>
