<script lang="ts">
  import { onDestroy } from "svelte";
  import type { ListSearch } from "../../../types";
  import { cn } from "../../../utils";
  import Input from "../input/input.svelte";
  import Select from "../select/select.svelte";

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
    class="w-auto min-w-0 flex-1 rounded-r-none focus-visible:z-10"
    type="search"
    placeholder="Search"
    bind:value={query.q}
    oninput={queue}
    aria-label="Search"
  />
  <Select
    class="-ml-px w-auto shrink-0 rounded-l-none"
    title="Search by"
    aria-label="Search by"
    bind:value={query.column}
    onchange={columnChanged}
  >
    {#each columns as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </Select>
</div>
