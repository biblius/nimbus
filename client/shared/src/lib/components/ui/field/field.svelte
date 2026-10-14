<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  import type { ValidationField } from "../../../api";
  import { cn } from "../../../utils";
  import Input from "../input/input.svelte";
  import Label from "../label/label.svelte";

  let {
    label,
    id,
    error,
    value = $bindable(""),
    class: className,
    dense = false,
    children,
    ...rest
  }: HTMLInputAttributes & {
    /**
     * The form item label.
     */
    label: string;
    /**
     * Server validation error for this field.
     * When set, the control is highlighted and the message is shown underneath.
     */
    error?: ValidationField;
    /**
     * The bindable field value.
     */
    value?: string;
    /**
     * Compact spacing and control height for dense forms.
     */
    dense?: boolean;
    /**
     * Can be used to inject other form items other than input, e.g. a select.
     * If present, renders whatever is in the snippet.
     */
    children?: Snippet;
  } = $props();

  const message = $derived(error?.message || error?.code);
</script>

<div class={dense ? "space-y-1" : "space-y-2"}>
  <Label for={id} class={dense ? "text-xs text-muted-foreground" : undefined}
    >{label}</Label
  >
  {#if children}
    {@render children()}
  {:else}
    <Input
      {id}
      {...rest}
      class={cn(
        dense && "h-8 shadow-none",
        message && "border-destructive focus-visible:ring-destructive",
        className,
      )}
      bind:value
      aria-invalid={message ? true : undefined}
    />
  {/if}
  {#if message}
    <p class="text-xs text-destructive">{message}</p>
  {/if}
</div>
