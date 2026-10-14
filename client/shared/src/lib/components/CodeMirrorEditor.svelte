<script lang="ts">
  import { onMount } from "svelte";
  import {
    getContent,
    init,
    setContent,
    setEditable,
    type InitOptions,
  } from "../codemirror.svelte";
  import { cn } from "../utils";
  import type { EditorView } from "codemirror";

  let {
    value = $bindable(""),
    editable = false,
    lineWrap = true,
    class: className = "",
    "aria-label": ariaLabel = "Text editor",
  }: {
    value?: string;
    editable?: boolean;
    lineWrap?: boolean;
    class?: string;
    "aria-label"?: string;
  } = $props();

  let host = $state<HTMLDivElement | null>(null);
  let view = $state<EditorView | null>(null);

  onMount(() => {
    if (!host) {
      return;
    }

    const options: InitOptions = {
      parent: host,
      doc: value,
      editable,
      lineWrap,
      onUpdate: (update) => {
        if (update.docChanged) {
          value = getContent(update.view);
        }
      },
    };

    view = init(options);

    return () => {
      view?.destroy();
      view = null;
    };
  });

  $effect(() => {
    if (!view) {
      return;
    }
    setEditable(view, editable);
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
    "min-h-0 w-full flex-1 overflow-hidden rounded-xl bg-zinc-900/80 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] ring-1 ring-white/10",
    editable && "ring-white/25",
    className,
  )}
  role="textbox"
  aria-label={ariaLabel}
  aria-readonly={!editable}
></div>
