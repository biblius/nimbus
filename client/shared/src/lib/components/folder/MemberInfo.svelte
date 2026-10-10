<script lang="ts">
  import { onMount } from "svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Member } from "../../types";

  let {
    folderId,
    members = $bindable(null),
  }: { folderId: string; members?: Member[] | null } = $props();

  let failed = $state(false);

  onMount(() => {
    session.client
      .listMembers(folderId)
      .then((next) => {
        members = next;
      })
      .catch((cause) => {
        failed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load members",
        );
      });
  });
</script>

<div class="space-y-1.5">
  <p class="text-sm font-medium">
    Members
    {#if members}
      <span class="font-normal text-muted-foreground">
        {members.length}
      </span>
    {/if}
  </p>
  {#if failed}
    <p
      class="rounded-lg border bg-card px-3 py-2 text-sm text-muted-foreground"
    >
      Could not load members.
    </p>
  {:else if !members}
    <p
      class="rounded-lg border bg-card px-3 py-2 text-sm text-muted-foreground"
    >
      Loading…
    </p>
  {:else if members.length === 0}
    <p
      class="rounded-lg border bg-card px-3 py-2 text-sm text-muted-foreground"
    >
      No members.
    </p>
  {:else}
    <ul class="divide-y overflow-hidden rounded-lg border bg-card">
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
</div>
