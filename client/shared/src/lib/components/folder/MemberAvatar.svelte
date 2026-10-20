<script lang="ts">
  import { cn } from "../../utils";

  const PALETTE = [
    "bg-sky-600",
    "bg-emerald-600",
    "bg-violet-600",
    "bg-amber-600",
    "bg-rose-600",
    "bg-teal-600",
    "bg-indigo-600",
    "bg-orange-600",
  ] as const;

  let {
    name,
    seed = name,
    class: className,
  }: {
    name: string;
    /** Stable identity for color hashing; defaults to `name`. */
    seed?: string;
    class?: string;
  } = $props();

  const initials = $derived.by(() => {
    const parts = name.trim().split(/\s+/).filter(Boolean);
    if (parts.length >= 2) {
      return (parts[0]![0]! + parts[1]![0]!).toUpperCase();
    }
    return name.trim().slice(0, 2).toUpperCase() || "?";
  });

  const colorClass = $derived.by(() => {
    let hash = 0;
    for (let i = 0; i < seed.length; i++) {
      hash = (Math.imul(31, hash) + seed.charCodeAt(i)) | 0;
    }
    return PALETTE[Math.abs(hash) % PALETTE.length]!;
  });
</script>

<span
  class={cn(
    "inline-flex size-9 shrink-0 items-center justify-center rounded-full text-xs font-semibold text-white",
    colorClass,
    className,
  )}
  aria-hidden="true"
>
  {initials}
</span>
