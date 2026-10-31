<script lang="ts">
  import { Eye, EyeOff } from "@lucide/svelte";
  import type { HTMLInputAttributes } from "svelte/elements";
  import { cn } from "../../../utils";

  let {
    class: className,
    value = $bindable(),
    ref = $bindable(),
    type,
    ...rest
  }: HTMLInputAttributes & { ref?: HTMLInputElement } = $props();

  let visible = $state(false);

  const isPassword = $derived(type === "password");
  const inputType = $derived(isPassword && visible ? "text" : type);
</script>

{#if isPassword}
  <div class="relative w-full min-w-0 flex-1">
    <input
      bind:this={ref}
      bind:value
      type={inputType}
      class={cn(
        "flex h-9 w-full rounded-md border border-input bg-field px-3 py-1 pr-9 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive",
        className,
      )}
      {...rest}
    />
    <button
      type="button"
      class="absolute inset-y-0 right-0 flex items-center justify-center px-2.5 text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
      aria-label={visible ? "Hide password" : "Show password"}
      aria-pressed={visible}
      disabled={rest.disabled}
      onclick={() => (visible = !visible)}
    >
      {#if visible}
        <EyeOff size={16} aria-hidden="true" />
      {:else}
        <Eye size={16} aria-hidden="true" />
      {/if}
    </button>
  </div>
{:else}
  <input
    bind:this={ref}
    bind:value
    type={type}
    class={cn(
      "flex h-9 w-full rounded-md border border-input bg-field px-3 py-1 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive",
      className,
    )}
    {...rest}
  />
{/if}
