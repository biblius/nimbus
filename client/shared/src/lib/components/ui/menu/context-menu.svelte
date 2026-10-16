<script lang="ts">
  import type { Snippet } from "svelte";
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

  setMenu({ close: () => (open = false) });
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
    role="menu"
    class={cn(
      "fixed z-50 w-44 rounded-xl border bg-card p-1 shadow-lg",
      className,
    )}
    style:left="{x}px"
    style:top="{y}px"
  >
    {@render children()}
  </div>
{/if}
