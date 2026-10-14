<script lang="ts">
  import { onMount } from "svelte";
  import { formatBytes, formatQuota } from "../format";
  import { navigate } from "../router";
  import { session } from "../session.svelte";
  import type { Root } from "../types";

  let {
    root,
    selected = false,
    inset = false,
  }: {
    root: Root;
    selected?: boolean;
    /** Shared roots sit under the section heading. */
    inset?: boolean;
  } = $props();

  let usage = $state<number | undefined>();
  let usageGeneration = 0;

  const limited = $derived(root.quota > 0);

  const ratio = $derived.by(() => {
    if (!limited || usage === undefined) {
      return null;
    }
    return Math.min(1, usage / root.quota);
  });

  const usageLabel = $derived.by(() => {
    if (!limited || usage === undefined) {
      return formatQuota(root.quota);
    }
    return `${formatBytes(usage)} / ${formatQuota(root.quota)}`;
  });

  onMount(() => {
    if (root.quota <= 0) {
      return;
    }

    const id = root.id;
    const current = ++usageGeneration;

    session.client
      .folderUsage(id)
      .then((bytes) => {
        if (current === usageGeneration) {
          usage = bytes;
        }
      })
      .catch(() => {
        if (current === usageGeneration) {
          usage = undefined;
        }
      });
  });
</script>

<button
  type="button"
  class={[
    "flex min-h-11 w-full flex-col justify-center gap-1.5 rounded-md py-2 text-left text-sm",
    inset ? "pr-3 pl-6" : "px-3",
    selected
      ? "bg-accent font-medium text-accent-foreground"
      : "hover:bg-accent",
  ]}
  aria-current={selected ? "page" : undefined}
  onclick={() => {
    session.menuOpen = false;
    navigate("/files/:root_id", { params: { root_id: root.id } });
  }}
>
  <span class="flex w-full min-w-0 items-center gap-2">
    <span class="min-w-0 flex-1 truncate">{root.name}</span>
    <span
      class="shrink-0 text-xs font-normal tabular-nums {ratio != null &&
      ratio > 0.9
        ? 'text-destructive'
        : 'text-muted-foreground'}"
    >
      {usageLabel}
    </span>
  </span>
  {#if limited}
    <span
      class="block h-1 w-full overflow-hidden rounded-full bg-input"
      aria-hidden="true"
    >
      {#if ratio != null}
        <span
          class="block h-full rounded-full transition-[width] duration-500 {ratio >
          0.9
            ? 'bg-destructive'
            : 'bg-primary'}"
          style:width="{ratio * 100}%"
        ></span>
      {/if}
    </span>
  {/if}
</button>
