<script lang="ts">
  import type { Snippet } from "svelte";
  import PanelShell from "../../components/PanelShell.svelte";
  import { isActive, navigate } from "../../router";
  import { session } from "../../session.svelte";

  let { children }: { children: Snippet } = $props();

  const folderLinks = [
    { href: "/admin/folders/private", label: "Private" },
    { href: "/admin/folders/shared", label: "Shared" },
  ] as const;

  $effect(() => {
    // Ensure non-admins get redirected back to the users page
    if (session.user?.role !== "admin") navigate("/", { replace: true });
  });
</script>

{#if session.user?.role === "admin"}
  <PanelShell bind:open={session.menuOpen} label="Configuration">
    {#snippet sidebar()}
      <!-- SIDEBAR -->

      <nav class="flex flex-col gap-1 p-3" aria-label="Configuration">
        <p
          class="hidden px-3 pb-2 text-center text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
        >
          Administration
        </p>

        <!-- USERS -->

        <a
          href={"/admin/users"}
          class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {isActive(
            '/admin/users',
          )
            ? 'bg-accent font-medium text-accent-foreground'
            : 'hover:bg-accent'}"
          aria-current={isActive("/admin/users") ? "page" : undefined}
          onclick={() => (session.menuOpen = false)}
        >
          Users
        </a>

        <!-- FOLDERS -->

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
              onclick={() => (session.menuOpen = false)}
            >
              {link.label}
            </a>
          {/each}
        </div>
      </nav>
    {/snippet}

    <!-- PAGE CONTENT -->

    <div class="p-4 sm:p-6">
      <div class="mx-auto max-w-5xl">
        {@render children()}
      </div>
    </div>
  </PanelShell>
{/if}
