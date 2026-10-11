<script lang="ts">
  import { onMount } from "svelte";
  import { navigate } from "../router";

  onMount(() => {
    navigate("/files", { replace: true });
  });
</script>
