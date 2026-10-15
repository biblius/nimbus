<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import type { HTMLSelectAttributes } from "svelte/elements";
  import { cn } from "../../../utils";

  let {
    class: className,
    value = $bindable(),
    ref = $bindable(),
    children,
    ...rest
  }: HTMLSelectAttributes & { ref?: HTMLSelectElement } = $props();
</script>

<div class={cn("relative flex h-9 w-full rounded-md", className)}>
  <select
    bind:this={ref}
    bind:value
    class="peer h-full w-full cursor-pointer appearance-none rounded-[inherit] border border-input bg-field py-0 pr-8 pl-3 text-sm shadow-sm transition-colors outline-none hover:bg-accent focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive"
    {...rest}
  >
    {@render children?.()}
  </select>
  <ChevronDown
    size={14}
    class="pointer-events-none absolute top-1/2 right-2.5 z-10 -translate-y-1/2 text-muted-foreground peer-disabled:opacity-50"
    aria-hidden="true"
  />
</div>
