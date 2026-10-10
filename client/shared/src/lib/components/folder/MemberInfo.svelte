<script lang="ts">
  import InfiniteList from "../ui/infinite-list/infinite-list.svelte";
  import { session } from "../../session.svelte";
  import type { Member } from "../../types";

  const searchColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
  ];

  let { folderId }: { folderId: string } = $props();

  let total = $state<number>(0);
</script>

<div class="space-y-1.5">
  <p class="text-sm font-medium">
    Members
    {#if total > 0}
      <span class="font-normal text-muted-foreground">
        {total}
      </span>
    {/if}
  </p>

  <InfiniteList
    columns={searchColumns}
    bind:total
    errorMessage="Could not load members."
    load={(query) => session.client.listMembers(folderId, query)}
  >
    {#snippet children(members: Member[])}
      {#if members.length === 0}
        <p class="px-3 py-2 text-sm text-muted-foreground">No members.</p>
      {:else}
        <ul class="divide-y">
          {#each members as member (member.id)}
            <li class="flex items-center justify-between gap-3 px-3 py-2">
              <span class="min-w-0">
                <span class="block truncate text-sm font-medium">
                  {member.username}
                </span>
                {#if member.email}
                  <span class="block truncate text-xs text-muted-foreground">
                    {member.email}
                  </span>
                {/if}
              </span>
              <span
                class="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-xs text-muted-foreground capitalize"
              >
                {member.role}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </InfiniteList>
</div>
