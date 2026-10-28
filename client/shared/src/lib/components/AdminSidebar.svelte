<script lang="ts">
  import { isActive } from "../router";
  import { session } from "../session.svelte";

  const folderLinks = [
    { href: "/admin/folders/private", label: "Private" },
    { href: "/admin/folders/shared", label: "Shared" },
  ] as const;

  function close() {
    session.menuOpen = false;
  }
</script>

<nav class="flex flex-col gap-1 p-3" aria-label="Administration">
  <p
    class="hidden px-3 pb-2 text-center text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
  >
    Administration
  </p>

  <a
    href="/admin/settings"
    class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {isActive(
      '/admin/settings',
    )
      ? 'bg-accent font-medium text-accent-foreground'
      : 'hover:bg-accent'}"
    aria-current={isActive("/admin/settings") ? "page" : undefined}
    onclick={close}
  >
    Settings
  </a>

  <a
    href="/admin/users"
    class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {isActive(
      '/admin/users',
    )
      ? 'bg-accent font-medium text-accent-foreground'
      : 'hover:bg-accent'}"
    aria-current={isActive("/admin/users") ? "page" : undefined}
    onclick={close}
  >
    Users
  </a>

  <div
    class="mt-2 flex flex-col gap-1 border-t pt-2"
    role="group"
    aria-labelledby="folders-heading"
  >
    <p
      id="folders-heading"
      class="px-3 py-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase"
    >
      Folders
    </p>

    {#each folderLinks as link (link.href)}
      <a
        href={link.href}
        class="flex min-h-11 items-center rounded-md pr-3 pl-6 text-left text-sm {isActive.startsWith(
          link.href,
        )
          ? 'bg-accent font-medium text-accent-foreground'
          : 'hover:bg-accent'}"
        aria-current={isActive.startsWith(link.href) ? "page" : undefined}
        onclick={close}
      >
        {link.label}
      </a>
    {/each}
  </div>
</nav>
