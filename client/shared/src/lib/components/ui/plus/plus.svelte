<script lang="ts">
  let { rotated = $bindable(false) }: { rotated?: boolean } = $props();
</script>

<svg
  viewBox="0 0 20 20"
  class="size-4 transition-transform {rotated ? 'rotate-45' : ''}"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  aria-hidden="true"
>
  <path d="M10 4v12M4 10h12" />
</svg>
