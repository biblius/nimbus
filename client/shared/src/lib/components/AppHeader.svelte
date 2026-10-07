<script lang="ts">
  import type { UserView } from "../types";
  import Button from "./ui/button/button.svelte";

  let {
    user,
    mode,
    menuOpen,
    onToggleMenu,
    onToggleMode,
    onLogout,
  }: {
    user: UserView;
    mode: "user" | "admin";
    menuOpen: boolean;
    onToggleMenu: () => void;
    onToggleMode: () => void;
    onLogout: () => void;
  } = $props();

  const admin = $derived(user.role === "admin");
</script>

<header
  class="sticky top-0 z-50 flex h-14 items-center gap-2 border-b bg-card px-3 sm:px-4"
>
  <button
    type="button"
    class="inline-flex size-11 shrink-0 items-center justify-center rounded-md hover:bg-accent md:hidden"
    aria-label={menuOpen ? "Close menu" : "Open menu"}
    aria-expanded={menuOpen}
    onclick={onToggleMenu}
  >
    <svg
      viewBox="0 0 20 20"
      class="size-5"
      fill="none"
      stroke="currentColor"
      aria-hidden="true"
    >
      {#if menuOpen}
        <path
          d="M5 5l10 10M15 5L5 15"
          stroke-width="1.75"
          stroke-linecap="round"
        />
      {:else}
        <path
          d="M3 5h14M3 10h14M3 15h14"
          stroke-width="1.75"
          stroke-linecap="round"
        />
      {/if}
    </svg>
  </button>

  <div class="min-w-0 flex-1">
    <p class="truncate text-sm font-medium">{user.username}</p>
    <p class="truncate text-xs text-muted-foreground capitalize">{user.role}</p>
  </div>

  {#if admin}
    <Button
      type="button"
      variant="outline"
      class="h-11 shrink-0 px-3 md:h-9"
      aria-label={mode === "user"
        ? "Switch to the admin panel"
        : "Switch to your files"}
      onclick={onToggleMode}
    >
      {mode === "user" ? "Admin" : "Files"}
    </Button>
  {/if}

  <Button
    type="button"
    variant="outline"
    class="h-11 shrink-0 px-3 md:h-9"
    onclick={onLogout}
  >
    Log out
  </Button>
</header>
