<script lang="ts">
  import PagedList from "../ui/paged-list/paged-list.svelte";
  import { session } from "../../session.svelte";
  import { toastError } from "../../toast";
  import type { Member } from "../../types";

  let { folderId }: { folderId: string } = $props();

  let members = $state<Member[] | null>(null);

  let loadAmount = $state(10);
  let total = $state<number | null>(null);
  let failed = $state(false);
  let page = 1;
  let finished = false;
  let pending: Promise<boolean> | null = null;

  async function loadNext(): Promise<boolean> {
    if (finished) {
      return false;
    }
    if (pending) {
      return pending;
    }

    const current = page;

    pending = session.client
      .listMembers(folderId, current, loadAmount)
      .then((listed) => {
        if (!members) {
          members = [];
        }

        for (const member of listed.items) {
          if (members.some((existing) => existing.id === member.id)) {
            continue;
          }
          members.push(member);
        }

        total = listed.total;
        page = current + 1;
        finished = listed.items.length === 0 || members.length >= listed.total;

        return !finished;
      })
      .catch((cause) => {
        finished = true;
        failed = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load members",
        );
        return false;
      })
      .finally(() => {
        pending = null;
      });

    return pending;
  }
</script>

<div class="space-y-1.5">
  <p class="text-sm font-medium">
    Members
    {#if total != null}
      <span class="font-normal text-muted-foreground">
        {total}
      </span>
    {/if}
  </p>
  <PagedList {loadNext} bind:loadAmount>
    {#if failed && !members}
      <p class="px-3 py-2 text-sm text-muted-foreground">
        Could not load members.
      </p>
    {:else if members && members.length === 0}
      <p class="px-3 py-2 text-sm text-muted-foreground">No members.</p>
    {:else}
      <ul class="divide-y">
        {#each members ?? [] as member (member.id)}
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
  </PagedList>
</div>
