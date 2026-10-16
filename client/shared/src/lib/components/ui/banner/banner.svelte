<script lang="ts" module>
  import { tv, type VariantProps } from "tailwind-variants";

  export const bannerVariants = tv({
    base: "flex items-center gap-2.5 rounded-md border-l-2 px-3 py-2 text-xs",
    variants: {
      tone: {
        warning: "border-l-warning bg-warning/8",
        info: "border-l-primary bg-primary/8",
      },
    },
    defaultVariants: {
      tone: "warning",
    },
  });

  export type BannerTone = VariantProps<typeof bannerVariants>["tone"];
</script>

<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  import { cn } from "../../../utils";

  let {
    class: className,
    tone = "warning",
    icon,
    action,
    children,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    tone?: BannerTone;
    icon?: Snippet;
    action?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div
  role="status"
  class={cn(bannerVariants({ tone }), className)}
  {...rest}
>
  {#if icon}
    <span
      class={cn(
        "flex shrink-0 items-center [&>svg]:size-3.5",
        tone === "warning" ? "text-warning" : "text-primary",
      )}
    >
      {@render icon()}
    </span>
  {/if}
  <div class="min-w-0 flex-1 leading-snug text-foreground">
    {@render children?.()}
  </div>
  {#if action}
    <div class="flex shrink-0 empty:hidden">
      {@render action()}
    </div>
  {/if}
</div>
