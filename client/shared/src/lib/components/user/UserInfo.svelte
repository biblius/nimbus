<script lang="ts">
  import { type UserView } from "../../types";

  let { user }: { user: UserView } = $props();
</script>

<dl
  class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-6 gap-y-2 text-sm"
>
  <dt class="text-muted-foreground">Username</dt>
  <dd class="truncate">{user.username}</dd>
  <dt class="text-muted-foreground">Email</dt>
  <dd class="truncate">{user.email ?? "Not set"}</dd>
  <dt class="text-muted-foreground">Role</dt>
  <dd>
    <span
      class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium capitalize {user.role ===
      'admin'
        ? 'bg-primary/15 text-primary'
        : 'bg-muted text-muted-foreground'}"
    >
      {user.role}
    </span>
  </dd>
</dl>
