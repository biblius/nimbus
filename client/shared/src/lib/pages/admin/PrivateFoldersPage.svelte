<script lang="ts">
  import FoldersView from "../../components/folder/FoldersView.svelte";
  import { session } from "../../session.svelte";
</script>

<FoldersView load={() => session.client.listPrivateFolders()} />
