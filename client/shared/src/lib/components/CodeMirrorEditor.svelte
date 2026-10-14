<script lang="ts">
  import { onMount } from "svelte";
  import {
    editorPrefs,
    getContent,
    init,
    setContent,
    setEditable,
    setLineWrap,
    setVim,
    setVimExHandlers,
    type InitOptions,
  } from "../codemirror.svelte";
  import { cn } from "../utils";
  import type { EditorView } from "@codemirror/view";

  let {
    value = $bindable(""),
    editable = false,
    class: className = "",
    "aria-label": ariaLabel = "Text editor",
    onWrite,
    onQuit,
    onWriteHint,
  }: {
    value?: string;
    editable?: boolean;
    class?: string;
    "aria-label"?: string;
    onWrite?: () => void;
    onQuit?: () => void;
    onWriteHint?: () => void;
  } = $props();

  let host = $state<HTMLDivElement | null>(null);
  let view = $state<EditorView | null>(null);

  export function toggleWrap() {
    if (!view) {
      return;
    }
    setLineWrap(view, !editorPrefs.lineWrap);
  }

  export function toggleVim() {
    if (!view) {
      return;
    }
    setVim(view, !editorPrefs.vim);
  }

  onMount(() => {
    if (!host) {
      return;
    }

    const options: InitOptions = {
      parent: host,
      doc: value,
      editable,
      lineWrap: editorPrefs.lineWrap,
      // VIM only while editing — view mode keeps a normal caret.
      vim: editable && editorPrefs.vim,
      onUpdate: (update) => {
        if (update.docChanged) {
          value = getContent(update.view);
        }
      },
    };

    view = init(options);

    return () => {
      setVimExHandlers(null);
      view?.destroy();
      view = null;
    };
  });

  $effect(() => {
    if (!view) {
      return;
    }
    setEditable(view, editable);
    setVim(view, editable && editorPrefs.vim, { persist: false });
    if (editable) {
      setVimExHandlers({
        writeQuit: () => onWrite?.(),
        quit: () => onQuit?.(),
        writeHint: () => onWriteHint?.(),
      });
      view.focus();
    } else {
      setVimExHandlers(null);
    }
  });

  $effect(() => {
    if (!view) {
      return;
    }
    setContent(view, value);
  });
</script>

<div
  bind:this={host}
  class={cn(
    "min-h-0 w-full flex-1 overflow-hidden rounded-xl bg-zinc-900 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] ring-1 ring-white/10",
    editable && "ring-white/25",
    className,
  )}
  role="textbox"
  aria-label={ariaLabel}
  aria-readonly={!editable}
></div>
