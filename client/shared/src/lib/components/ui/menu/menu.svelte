<script lang="ts" module>
  export type MenuTriggerProps = {
    "aria-haspopup": "menu";
    "aria-expanded": boolean;
    onclick: () => void;
  };
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "../../../utils";
  import { setMenu } from "./context";

  let {
    open = $bindable(false),
    class: className,
    trigger,
    children,
  }: {
    open?: boolean;
    /** Classes for the menu panel, e.g. its width. */
    class?: string;
    /** Receives the props to spread on the trigger button, and whether the menu is open. */
    trigger: Snippet<[MenuTriggerProps, boolean]>;
    children: Snippet;
  } = $props();

  let root = $state<HTMLDivElement>();

  setMenu({ close: () => (open = false) });

  const triggerProps = $derived<MenuTriggerProps>({
    "aria-haspopup": "menu",
    "aria-expanded": open,
    onclick: () => (open = !open),
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

<div class="relative" bind:this={root}>
  {@render trigger(triggerProps, open)}

  {#if open}
    <div
      role="menu"
      class={cn(
        "absolute top-full right-0 z-50 mt-1.5 w-40 overflow-hidden rounded-xl border bg-card p-1 shadow-lg",
        className,
      )}
    >
      {@render children()}
    </div>
  {/if}
</div>
