<script lang="ts" module>
  export type MenuTriggerProps = {
    "aria-haspopup": "menu";
    "aria-expanded": boolean;
    onclick: () => void;
  };
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";
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

  let triggerRoot = $state<HTMLDivElement>();
  let panel = $state<HTMLDivElement>();
  let top = $state(0);
  let left = $state(0);

  setMenu({ close: () => (open = false) });

  const triggerProps = $derived<MenuTriggerProps>({
    "aria-haspopup": "menu",
    "aria-expanded": open,
    onclick: () => (open = !open),
  });

  /** Escape overflow/`transform` ancestors (toolstrip, virtual list). */
  const portal: Attachment<HTMLElement> = (node) => {
    document.body.appendChild(node);
    return () => {
      node.remove();
    };
  };

  function placePanel() {
    if (!triggerRoot || !panel) {
      return;
    }

    const anchor =
      triggerRoot.querySelector<HTMLElement>("[aria-haspopup='menu']") ??
      triggerRoot;
    const rect = anchor.getBoundingClientRect();
    const width = panel.offsetWidth;
    const height = panel.offsetHeight;
    const gap = 6;

    let nextLeft = rect.right - width;
    let nextTop = rect.bottom + gap;

    nextLeft = Math.min(
      Math.max(8, nextLeft),
      window.innerWidth - width - 8,
    );

    if (nextTop + height > window.innerHeight - 8) {
      nextTop = Math.max(8, rect.top - height - gap);
    }

    top = nextTop;
    left = nextLeft;
  }

  $effect(() => {
    if (!open) {
      return;
    }

    const id = requestAnimationFrame(() => placePanel());

    const onReposition = () => placePanel();
    window.addEventListener("resize", onReposition);
    // Capture scroll from nested scrollers (file list, etc.).
    window.addEventListener("scroll", onReposition, true);

    return () => {
      cancelAnimationFrame(id);
      window.removeEventListener("resize", onReposition);
      window.removeEventListener("scroll", onReposition, true);
    };
  });
</script>

<svelte:window
  onpointerdowncapture={(event) => {
    if (!open) {
      return;
    }

    const target = event.target as Node;
    if (triggerRoot?.contains(target) || panel?.contains(target)) {
      return;
    }

    open = false;
  }}
  onkeydown={(event) => {
    if (open && event.key === "Escape") {
      event.preventDefault();
      open = false;
    }
  }}
/>

<div class="inline-flex" bind:this={triggerRoot}>
  {@render trigger(triggerProps, open)}
</div>

{#if open}
  <div
    bind:this={panel}
    {@attach portal}
    role="menu"
    class={cn(
      "fixed z-[200] w-40 overflow-hidden rounded-xl border bg-card p-1 shadow-lg",
      className,
    )}
    style:top="{top}px"
    style:left="{left}px"
  >
    {@render children()}
  </div>
{/if}
