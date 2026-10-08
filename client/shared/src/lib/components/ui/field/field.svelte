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
     * Can be used to inject other form items other than input, e.g. a select.
     * If present, renders whatever is in the snippet.
     */
    children?: Snippet;
  } = $props();

  const message = $derived(error?.message || error?.code);
</script>

<div class="space-y-2">
  <Label for={id}>{label}</Label>
  {#if children}
    {@render children()}
  {:else}
    <Input
      {id}
      {...rest}
      class={cn(
        message && "border-destructive focus-visible:ring-destructive",
        className,
      )}
      bind:value
      aria-invalid={message ? true : undefined}
    />
  {/if}
  {#if message}
    <p class="text-sm text-destructive">{message}</p>
  {/if}
</div>
