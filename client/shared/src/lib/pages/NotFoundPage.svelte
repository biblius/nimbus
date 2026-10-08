<script lang="ts">
  import Button from "../components/ui/button/button.svelte";
  import { navigate } from "../router";
</script>

<main class="flex min-h-screen flex-col items-center justify-center gap-4 p-6">
  <p class="text-sm text-muted-foreground">This page does not exist.</p>
  <Button type="button" onclick={() => navigate("/")}>Go to your files</Button>
</main>
