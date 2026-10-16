<script lang="ts">
  import { formatDatetime } from "../../format";
  import { type UserViewFull } from "../../types";

  let { user }: { user: UserViewFull } = $props();
</script>

<dl
  class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-6 gap-y-2 text-sm"
>
  <dt class="text-muted-foreground">ID</dt>
  <dd class="truncate font-mono text-xs leading-5">{user.id}</dd>
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
  <dt class="text-muted-foreground">2FA</dt>
  <dd>
    <span
      class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] leading-none font-medium {user.otpEnabled
        ? 'bg-success/15 text-success'
        : 'bg-warning/15 text-warning'}"
    >
      {user.otpEnabled ? "Enabled" : "Disabled"}
    </span>
  </dd>
  <dt class="text-muted-foreground">Created</dt>
  <dd>{formatDatetime(user.createdAt)}</dd>
  <dt class="text-muted-foreground">Updated</dt>
  <dd>{formatDatetime(user.updatedAt)}</dd>
</dl>
