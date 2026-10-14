<script lang="ts">
  import { FastForward, Info, Pencil, TextWrap } from "@lucide/svelte";
  import { editorPrefs } from "../../codemirror.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { FileEntry } from "../../types";
  import { cn } from "../../utils";
  import CodeMirrorEditor from "../CodeMirrorEditor.svelte";

  const WQ_HINT_MS = 2500;

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
  let wqHintFlash = $state(false);
  let editor = $state<{ toggleWrap: () => void; toggleVim: () => void } | null>(
    null,
  );

  let wqHintTimer: ReturnType<typeof setTimeout> | undefined;

  const chip =
    "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium tracking-wide transition-colors";

  function startEdit() {
    draft = text;
    editing = true;
  }

  function cancelEdit() {
    draft = text;
    editing = false;
    clearWqHint();
  }

  function clearWqHint() {
    wqHintFlash = false;
    if (wqHintTimer !== undefined) {
      clearTimeout(wqHintTimer);
      wqHintTimer = undefined;
    }
  }

  function flashWqHint() {
    wqHintFlash = true;
    if (wqHintTimer !== undefined) {
      clearTimeout(wqHintTimer);
    }
    wqHintTimer = setTimeout(() => {
      wqHintFlash = false;
      wqHintTimer = undefined;
    }, WQ_HINT_MS);
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
    clearWqHint();

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
      clearWqHint();
    };
  });
</script>

<div class="flex h-full w-full flex-col gap-3">
  {#if !loading && !error}
    <header
      class="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-xl bg-zinc-900/90 px-3 py-2 ring-1 ring-white/10 backdrop-blur-md"
    >
      <div class="flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          class={cn(
            chip,
            editorPrefs.lineWrap
              ? "bg-white/15 text-white"
              : "text-white/55 hover:bg-white/10 hover:text-white",
          )}
          aria-pressed={editorPrefs.lineWrap}
          aria-label="Toggle line wrapping"
          onclick={() => editor?.toggleWrap()}
        >
          <TextWrap size={14} strokeWidth={2} />
          Wrap
        </button>

        {#if editing}
          <button
            type="button"
            class={cn(
              chip,
              editorPrefs.vim
                ? "bg-white/15 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
            aria-pressed={editorPrefs.vim}
            aria-label="Toggle VIM mode"
            onclick={() => editor?.toggleVim()}
          >
            <FastForward size={14} strokeWidth={2} />
            VIM
          </button>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        {#if editing}
          {#if editorPrefs.vim}
            <span
              class={cn(
                "inline-flex items-center gap-1.5 font-mono text-xs transition-colors duration-300",
                wqHintFlash ? "text-white" : "text-white/45",
              )}
            >
              {#if wqHintFlash}
                <Info size={14} strokeWidth={2} class="shrink-0" />
              {/if}
              :wq to save · :q to cancel
            </span>
          {:else}
            <button
              type="button"
              class="h-8 rounded-md px-3 text-xs font-medium text-white/55 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-50"
              disabled={saving}
              onclick={cancelEdit}
            >
              Discard
            </button>
            <button
              type="button"
              class="h-8 rounded-md bg-[#0082c9] px-3.5 text-xs font-semibold text-white shadow-sm transition-colors hover:bg-[#0090de] disabled:opacity-50"
              disabled={saving}
              onclick={save}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          {/if}
        {:else}
          <button
            type="button"
            class="inline-flex h-8 items-center gap-1.5 rounded-md bg-white/10 px-3 text-xs font-medium text-white transition-colors hover:bg-white/16"
            onclick={startEdit}
          >
            <Pencil size={14} strokeWidth={2} />
            Edit
          </button>
        {/if}
      </div>
    </header>
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
      bind:this={editor}
      bind:value={draft}
      editable={editing}
      aria-label={editing ? `Edit ${name}` : name}
      onWrite={save}
      onQuit={cancelEdit}
      onWriteHint={flashWqHint}
    />
  {/if}
</div>
