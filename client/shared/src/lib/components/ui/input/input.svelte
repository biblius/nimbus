<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { cn } from "../../../utils";

  let {
    class: className,
    value = $bindable(),
    ...rest
  }: HTMLInputAttributes = $props();
</script>

<input
  bind:value
  class={cn(
    "flex h-9 w-full rounded-md border border-input bg-field px-3 py-1 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
    className,
  )}
  {...rest}
/>
