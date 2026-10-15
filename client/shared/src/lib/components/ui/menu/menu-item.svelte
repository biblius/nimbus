<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { cn } from "../../../utils";
  import { getMenu } from "./context";

  let {
    class: className,
    closeOnSelect = true,
    onclick,
    children,
    ...rest
  }: HTMLButtonAttributes & {
    /** Close the menu after this item is clicked. */
    closeOnSelect?: boolean;
  } = $props();

  const menu = getMenu();
</script>

<button
  type="button"
  role="menuitem"
  class={cn(
    "flex min-h-11 w-full items-center gap-2 rounded-md px-3 text-left text-sm hover:bg-accent focus-visible:bg-accent focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50 md:min-h-9 [&_svg]:shrink-0 [&_svg]:text-muted-foreground",
    className,
  )}
  onclick={(event) => {
    if (closeOnSelect) {
      menu.close();
    }
    onclick?.(event);
  }}
  {...rest}
>
  {@render children?.()}
</button>
