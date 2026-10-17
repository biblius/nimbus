<script lang="ts">
  import { Link2 } from "@lucide/svelte";
  import { cn } from "../../utils";

  let {
    size = "md",
    class: className,
  }: {
    /** Matches list compact / default / tile densities. */
    size?: "sm" | "md" | "lg";
    class?: string;
  } = $props();

  const badgeClass = $derived(
    size === "sm" ? "size-3" : size === "lg" ? "size-4" : "size-3.5",
  );
  const iconPx = $derived(size === "sm" ? 8 : size === "lg" ? 10 : 9);
</script>

<span
  class={cn(
    "pointer-events-none absolute -right-0.5 -bottom-0.5 z-[1] flex items-center justify-center rounded-full bg-background text-primary shadow-sm ring-1 ring-primary/25",
    badgeClass,
    className,
  )}
  title="Shared"
  aria-label="Publicly shared"
>
  <Link2 size={iconPx} strokeWidth={2.5} />
</span>
