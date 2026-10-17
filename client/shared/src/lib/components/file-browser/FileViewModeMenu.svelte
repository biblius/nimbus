<script lang="ts">
  import {
    AlignJustify,
    Check,
    Grid2x2,
    LayoutGrid,
    List,
  } from "@lucide/svelte";
  import { type FilesViewMode, kv } from "../../storage";
  import { cn } from "../../utils";
  import Button from "../ui/button/button.svelte";
  import MenuItem from "../ui/menu/menu-item.svelte";
  import Menu from "../ui/menu/menu.svelte";

  let {
    mode = $bindable<FilesViewMode>(),
    class: className,
  }: {
    mode: FilesViewMode;
    class?: string;
  } = $props();

  const modes: {
    value: FilesViewMode;
    label: string;
    Icon: typeof List;
  }[] = [
    { value: "list", label: "List", Icon: List },
    { value: "compact", label: "Compact list", Icon: AlignJustify },
    { value: "tiles-small", label: "Small tiles", Icon: LayoutGrid },
    { value: "tiles-large", label: "Large tiles", Icon: Grid2x2 },
  ];

  const CurrentIcon = $derived(
    modes.find((item) => item.value === mode)?.Icon ?? List,
  );

  function setMode(next: FilesViewMode) {
    mode = next;
    kv.filesViewMode.set(next);
  }
</script>

<div class={cn(className)}>
  <Menu class="w-52">
    {#snippet trigger(props)}
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="View mode"
        title="View mode"
        {...props}
      >
        <CurrentIcon size={16} />
      </Button>
    {/snippet}

    {#each modes as item (item.value)}
      {@const Icon = item.Icon}
      {@const selected = mode === item.value}
      <MenuItem
        class={selected ? "font-medium" : undefined}
        onclick={() => setMode(item.value)}
      >
        <Icon size={16} />
        {item.label}
        {#if selected}
          <Check size={15} class="ml-auto" />
        {/if}
      </MenuItem>
    {/each}
  </Menu>
</div>
