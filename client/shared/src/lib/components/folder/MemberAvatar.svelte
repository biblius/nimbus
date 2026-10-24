<script lang="ts">
  import { cn } from "../../utils";

  let {
    name,
    class: className,
  }: {
    name: string;
    class?: string;
  } = $props();

  const initials = $derived.by(() => {
    const parts = name.trim().split(/\s+/).filter(Boolean);
    if (parts.length >= 2) {
      return (parts[0]![0]! + parts[1]![0]!).toUpperCase();
    }
    return name.trim().slice(0, 2).toUpperCase() || "?";
  });
</script>

<span
  class={cn(
    "inline-flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-[11px] font-medium tracking-wide text-muted-foreground",
    className,
  )}
  aria-hidden="true"
>
  {initials}
</span>
