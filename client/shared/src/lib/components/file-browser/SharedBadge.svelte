<script lang="ts">
  import { Link2 } from "@lucide/svelte";
  import { formatDatetime } from "../../format";
  import { cn } from "../../utils";

  let {
    size = "md",
    class: className,
    expiresAt,
  }: {
    /** Matches list compact / default / tile densities. */
    size?: "sm" | "md" | "lg";
    class?: string;
    /** When the public share expires; omit/null means no expiry. */
    expiresAt?: string | null;
  } = $props();

  const badgeClass = $derived(
    size === "sm" ? "size-4" : size === "lg" ? "size-6" : "size-5",
  );
  const iconPx = $derived(size === "sm" ? 10 : size === "lg" ? 14 : 12);

  const label = $derived(
    expiresAt == null
      ? "This file is publicly available until the end of time"
      : `This file is publicly available until ${formatDatetime(expiresAt)}`,
  );
</script>

<span
  class={cn(
    "absolute -right-1 -bottom-1 z-1 flex items-center justify-center rounded-full bg-primary text-primary-foreground shadow-md ring-2 ring-background",
    badgeClass,
    className,
  )}
  title={label}
  aria-label={label}
>
  <Link2 size={iconPx} strokeWidth={2.75} aria-hidden="true" />
</span>
