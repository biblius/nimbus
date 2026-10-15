<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { cn } from "../../../utils";

  let {
    class: className,
    checked = $bindable(false),
    ...rest
  }: Omit<HTMLInputAttributes, "type"> = $props();
</script>

<input
  type="checkbox"
  bind:checked
  class={cn(
    "size-4 shrink-0 cursor-pointer rounded border-input accent-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50",
    className,
  )}
  {...rest}
/>
