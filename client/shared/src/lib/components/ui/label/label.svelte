<script lang="ts">
  import type { HTMLLabelAttributes } from "svelte/elements";
  import { cn } from "../../../utils";

  let { class: className, children, ...rest }: HTMLLabelAttributes = $props();
</script>

<label class={cn("text-sm leading-none font-medium", className)} {...rest}>
  {@render children?.()}
</label>
