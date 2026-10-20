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

  /**
   * Last search that was reported to the parent.
   * Empty string means "no search filter" (matches InfiniteList's initial load).
   */
  let lastEmitted = "";

  function searchKey() {
    const q = query.q.trim();
    return q ? `${query.column}\0${q}` : "";
  }

  function emit() {
    const key = searchKey();
    if (key === lastEmitted) {
      return;
    }
    lastEmitted = key;
    onUpdate?.();
  }

  function queue() {
    if (timer) {
      clearTimeout(timer);
    }
    timer = setTimeout(emit, 250);
  }

  function columnChanged() {
    if (timer) {
      clearTimeout(timer);
    }
    // Column only affects the request when there is a non-empty query.
    emit();
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
