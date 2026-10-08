<script lang="ts">
  import type { Snippet } from "svelte";
  import PanelShell from "../../components/PanelShell.svelte";
  import { isActive, navigate } from "../../router";
  import { session } from "../../session.svelte";

  let { children }: { children: Snippet } = $props();

  const links = [
    { href: "/admin/users", label: "Users" },
    { href: "/admin/folders", label: "Folders" },
  ] as const;

  $effect(() => {
    // Ensure non-admins get redirected back to the users page
    if (session.user?.role !== "admin") navigate("/", { replace: true });
  });
</script>

{#if session.user?.role === "admin"}
  <PanelShell bind:open={session.menuOpen} label="Configuration">
    {#snippet sidebar()}
      <nav class="flex flex-col gap-1 p-3" aria-label="Configuration">
        <p
          class="hidden px-3 pb-2 text-xs font-medium tracking-wide text-muted-foreground uppercase md:block"
        >
          Configuration
        </p>
        {#each links as link (link.href)}
          <a
            href={link.href}
            class="flex min-h-11 items-center rounded-md px-3 text-left text-sm {isActive(
              link.href,
            )
              ? 'bg-accent font-medium text-accent-foreground'
              : 'hover:bg-accent'}"
            aria-current={isActive(link.href) ? "page" : undefined}
            onclick={() => (session.menuOpen = false)}
          >
            {link.label}
          </a>
        {/each}
      </nav>
    {/snippet}

    <div class="p-4 sm:p-6">
      <div class="mx-auto max-w-5xl">
        {@render children()}
      </div>
    </div>
  </PanelShell>
{/if}
