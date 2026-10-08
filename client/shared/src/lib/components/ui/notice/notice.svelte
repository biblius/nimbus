<script lang="ts">
  import { Info } from "@lucide/svelte";

  let { message }: { message: string } = $props();
</script>

<p
  class="flex items-start gap-2 rounded-lg border border-sky-200 bg-sky-50 px-3 py-2 text-sm text-sky-950"
  role="status"
>
  <Info class="mt-0.5 size-4 shrink-0 text-sky-600" aria-hidden="true" />
  <span>{message}</span>
</p>
