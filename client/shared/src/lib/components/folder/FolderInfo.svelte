<script lang="ts">
  import type { Member, Root, RootData, UserViewFull } from "../../types";
  import Chevron from "../ui/chevron/chevron.svelte";
  let {
    folder,
    selected = $bindable(false),
  }: { folder: Root; selected: boolean } = $props();
</script>

<button
  type="button"
  class="flex min-h-11 w-full items-center justify-between gap-3 px-3 py-2 text-left {selected
    ? 'bg-accent'
    : 'hover:bg-accent/60'}"
  aria-expanded={selected}
  onclick={() => {
    const next = selected ? null : folder.id;
    reset();
    selectedId = next;
    if (next) {
      showMembers(next).catch(() => {
        toastError("Could not load members");
      });
    }
  }}
>
  <span class="min-w-0">
    <span class="block truncate text-sm font-medium">{folder.name}</span>
    <span class="block truncate text-xs text-muted-foreground">
      {folder.owner ? "Private" : "Shared"}
      · {formatQuota(folder.quota)}
    </span>
    <span class="block truncate text-xs text-muted-foreground">
      Created {formatDatetime(folder.createdAt)}
      · Updated {formatDatetime(folder.updatedAt)}
    </span>
  </span>
  <Chevron expanded={selected} />
</button>
