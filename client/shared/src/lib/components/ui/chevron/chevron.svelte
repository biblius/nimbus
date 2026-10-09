<script lang="ts">
  let { expanded = false }: { expanded?: boolean } = $props();
</script>

<svg
  viewBox="0 0 20 20"
  class="size-4 shrink-0 text-muted-foreground transition-transform {expanded
    ? 'rotate-180'
    : ''}"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d="m5 8 5 5 5-5" />
</svg>
