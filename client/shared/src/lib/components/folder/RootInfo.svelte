<script lang="ts">
  import { formatDatetime, formatQuota } from "../../format";
  import type { Root } from "../../types";

  let {
    folder,
    usage,
    ownerName,
  }: { folder: Root; usage: string; ownerName?: string } = $props();
</script>

<div class="flex flex-wrap items-start justify-between gap-3">
  <dl
    class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm"
  >
    <dt class="text-muted-foreground">ID</dt>
    <dd class="truncate font-mono text-xs leading-5">{folder.id}</dd>
    <dt class="text-muted-foreground">Name</dt>
    <dd class="truncate">{folder.name}</dd>
    {#if folder.owner}
      <dt class="text-muted-foreground">Owner</dt>
      <dd class="truncate">{ownerName}</dd>
    {:else}
      <dt class="text-muted-foreground">Type</dt>
      <dd>Shared</dd>
    {/if}
    <dt class="text-muted-foreground">Quota</dt>
    <dd>{formatQuota(folder.quota)}</dd>
    <dt class="text-muted-foreground">Usage</dt>
    <dd>{usage}</dd>
    <dt class="text-muted-foreground">Created</dt>
    <dd>{formatDatetime(folder.createdAt)}</dd>
    <dt class="text-muted-foreground">Updated</dt>
    <dd>{formatDatetime(folder.updatedAt)}</dd>
  </dl>
</div>
