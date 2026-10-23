<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { cn } from "../../../utils";
  import { setMenu } from "./context";

  let {
    open = $bindable(false),
    x,
    y,
    class: className,
    children,
  }: {
    open?: boolean;
    /** Client X coordinate for the menu origin. */
    x: number;
    /** Client Y coordinate for the menu origin. */
    y: number;
    /** Classes for the menu panel, e.g. its width. */
    class?: string;
    children: Snippet;
  } = $props();

  let root = $state<HTMLDivElement>();
  let top = $state(0);
  let left = $state(0);

  setMenu({ close: () => (open = false) });

  /**
   * Mount on `document.body` so `position: fixed` is not trapped by
   * transformed ancestors (e.g. the virtualized file list) and so the
   * menu stacks above row action buttons (`…`).
   */
  const portal: Attachment<HTMLElement> = (node) => {
    document.body.appendChild(node);
    return () => {
      node.remove();
    };
  };

  function placePanel() {
    if (!root) {
      return;
    }

    const width = root.offsetWidth;
    const height = root.offsetHeight;

    left = Math.min(Math.max(8, x), window.innerWidth - width - 8);
    top = Math.min(Math.max(8, y), window.innerHeight - height - 8);
  }

  $effect(() => {
    if (!open) {
      return;
    }

    left = x;
    top = y;

    const id = requestAnimationFrame(() => placePanel());
    const onReposition = () => placePanel();
    window.addEventListener("resize", onReposition);

    return () => {
      cancelAnimationFrame(id);
      window.removeEventListener("resize", onReposition);
    };
  });
</script>

<svelte:window
  onpointerdowncapture={(event) => {
    if (open && root && !root.contains(event.target as Node)) {
      open = false;
    }
  }}
  onkeydown={(event) => {
    if (open && event.key === "Escape") {
      event.preventDefault();
      open = false;
    }
  }}
/>

{#if open}
  <div
    bind:this={root}
    {@attach portal}
    role="menu"
    class={cn(
      "fixed z-[200] w-44 rounded-xl border bg-card p-1 shadow-lg",
      className,
    )}
    style:left="{left}px"
    style:top="{top}px"
  >
    {@render children()}
  </div>
{/if}
