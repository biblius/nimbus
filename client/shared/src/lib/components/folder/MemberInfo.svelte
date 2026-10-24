<script lang="ts">
  import InfiniteList from "../ui/infinite-list/infinite-list.svelte";
  import { session } from "../../session.svelte";
  import type { Member } from "../../types";
  import AccessPills from "./AccessPills.svelte";
  import MemberAvatar from "./MemberAvatar.svelte";

  const searchColumns = [
    { value: "username", label: "Username" },
    { value: "email", label: "Email" },
  ];

  let { folderId }: { folderId: string } = $props();

  let total = $state<number>(0);
</script>

<section class="space-y-3">
  <div class="flex items-center gap-2">
    <p
      class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
    >
      People with access
    </p>
    {#if total > 0}
      <span
        class="inline-flex min-w-5 items-center justify-center rounded-md bg-muted px-1.5 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground"
      >
        {total}
      </span>
    {/if}
  </div>

  <div class="overflow-hidden rounded-lg border bg-card shadow-sm">
    <InfiniteList
      class="space-y-0"
      searchClass="border-b bg-muted/25 p-2"
      scrollerClass="max-h-80 rounded-none border-0"
      columns={searchColumns}
      bind:total
      errorMessage="Could not load members."
      load={(query) => session.client.listMembers(folderId, query)}
    >
      {#snippet children(members: Member[])}
        {#if members.length === 0}
          <p class="px-4 py-8 text-center text-sm text-muted-foreground">
            No members.
          </p>
        {:else}
          <ul class="divide-y divide-border/60">
            {#each members as member (member.id)}
              <li
                class="flex flex-col gap-2 px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between sm:gap-3"
              >
                <div class="flex min-w-0 items-center gap-3">
                  <MemberAvatar name={member.username} />
                  <span class="min-w-0">
                    <span class="block truncate text-sm font-medium">
                      {member.username}
                    </span>
                    {#if member.email}
                      <span
                        class="block truncate text-xs text-muted-foreground"
                      >
                        {member.email}
                      </span>
                    {/if}
                  </span>
                </div>
                <div
                  class="flex flex-wrap items-center gap-2 pl-11 sm:justify-end sm:pl-0"
                >
                  <AccessPills accessMode={member.accessMode} />
                  <span
                    class={[
                      "rounded-md px-1.5 py-0.5 text-[11px] font-medium capitalize",
                      member.role === "admin"
                        ? "bg-primary/10 text-primary"
                        : "bg-muted text-muted-foreground",
                    ]}
                  >
                    {member.role}
                  </span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {/snippet}
    </InfiniteList>
  </div>
</section>
