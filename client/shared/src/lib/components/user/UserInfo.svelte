<script lang="ts">
  import { formatDatetime } from "../../format";
  import { type UserViewFull } from "../../types";

  let { user }: { user: UserViewFull } = $props();
</script>

<div class="flex flex-wrap items-start justify-between gap-3">
  <dl
    class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm"
  >
    <dt class="text-muted-foreground">ID</dt>
    <dd class="truncate font-mono text-xs leading-5">
      {user.id}
    </dd>
    <dt class="text-muted-foreground">Username</dt>
    <dd class="truncate">{user.username}</dd>
    <dt class="text-muted-foreground">Email</dt>
    <dd class="truncate">{user.email ?? "-"}</dd>
    <dt class="text-muted-foreground">Role</dt>
    <dd class="capitalize">{user.role}</dd>
    <dt class="text-muted-foreground">Two-factor</dt>
    <dd>{user.otpEnabled ? "Enabled" : "Disabled"}</dd>
    <dt class="text-muted-foreground">Created</dt>
    <dd>{formatDatetime(user.createdAt)}</dd>
    <dt class="text-muted-foreground">Updated</dt>
    <dd>{formatDatetime(user.updatedAt)}</dd>
  </dl>
</div>
