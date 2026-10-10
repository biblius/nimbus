<script lang="ts">
  import { onDestroy } from "svelte";
  import type { ListSearch } from "../../../types";
  import { cn } from "../../../utils";
  import Input from "../input/input.svelte";

  export type QueryColumn = {
    value: string;
    label: string;
  };

  let {
    columns,
    query = $bindable({ column: "", q: "" }),
    onUpdate,
    class: className,
  }: {
    columns: QueryColumn[];
    /** Column and text sent with the next load. */
    query?: ListSearch;
    /** Called after the text settles, and immediately when the column changes. */
    onUpdate?: () => void;
    class?: string;
  } = $props();

  let timer: ReturnType<typeof setTimeout> | undefined;

  function queue() {
    if (timer) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      onUpdate?.();
    }, 250);
  }

  function columnChanged() {
    if (timer) {
      clearTimeout(timer);
    }
    onUpdate?.();
  }

  onDestroy(() => {
    if (timer) {
      clearTimeout(timer);
    }
  });
</script>

<div class={cn("flex min-w-0 items-center", className)}>
  <Input
    class="mr-0 min-w-0 w-auto flex-1 rounded-r-none"
    type="search"
    placeholder="Search"
    bind:value={query.q}
    oninput={queue}
    aria-label="Search"
  />
  <label class="flex shrink-0 items-center text-sm">
    <select
      class="ml-0 h-9 appearance-none rounded-md rounded-l-none border border-input bg-field px-2 text-sm shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
      bind:value={query.column}
      onchange={columnChanged}
    >
      {#each columns as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </label>
</div>
