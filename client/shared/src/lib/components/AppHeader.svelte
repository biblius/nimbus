<script lang="ts">
  import { CircleUserRound, LogOut, Moon, Sun } from "@lucide/svelte";
  import { theme, toggleTheme } from "../theme.svelte";
  import type { UserView } from "../types";
  import Button from "./ui/button/button.svelte";

  let accountOpen = $state(false);
  let accountMenu: HTMLDivElement | undefined = $state();

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

  function closeAccount() {
    accountOpen = false;
  }
</script>

<svelte:window
  onclick={(event) => {
    if (!accountOpen || !accountMenu) {
      return;
    }
    if (!accountMenu.contains(event.target as Node)) {
      closeAccount();
    }
  }}
  onkeydown={(event) => {
    if (event.key === "Escape") {
      closeAccount();
    }
  }}
/>

<header
  class="sticky top-0 z-50 flex h-14 items-center gap-2 border-b bg-card/95 px-3 backdrop-blur-md sm:px-4"
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

  <img
    src="/favicon.png"
    alt="Nimbus"
    class="size-8 shrink-0 rounded-lg bg-primary p-0.5"
  />

  <div class="min-w-0 flex-1">
    <p class="truncate text-sm font-medium">{user.username}</p>
    <p class="truncate text-xs text-muted-foreground capitalize">{user.role}</p>
  </div>

  <div class="relative" bind:this={accountMenu}>
    <Button
      type="button"
      variant="outline"
      class="size-11 shrink-0 p-0 md:size-9"
      aria-label="Open account menu"
      aria-haspopup="menu"
      aria-expanded={accountOpen}
      onclick={(event) => {
        event.stopPropagation();
        accountOpen = !accountOpen;
      }}
    >
      <CircleUserRound size={18} />
    </Button>

    {#if accountOpen}
      <div
        role="menu"
        class="absolute top-full right-0 z-50 mt-1 w-44 rounded-lg border bg-card p-1 shadow-lg"
      >
        {#if admin}
          <button
            type="button"
            role="menuitem"
            class="flex min-h-11 w-full items-center gap-2 rounded-md px-3 text-left text-sm hover:bg-accent md:min-h-9"
            aria-label={mode === "user"
              ? "Switch to the admin panel"
              : "Switch to your files"}
            onclick={() => {
              closeAccount();
              onToggleMode();
            }}
          >
            {mode === "user" ? "Admin panel" : "User panel"}
          </button>
        {/if}

        <button
          type="button"
          role="menuitem"
          class="flex min-h-11 w-full items-center gap-2 rounded-md px-3 text-left text-sm hover:bg-accent md:min-h-9"
          onclick={() => toggleTheme()}
        >
          {#if theme.mode === "dark"}
            <Sun size={16} class="shrink-0 text-muted-foreground" />
            Light mode
          {:else}
            <Moon size={16} class="shrink-0 text-muted-foreground" />
            Dark mode
          {/if}
        </button>

        <div class="my-1 border-t"></div>

        <button
          type="button"
          role="menuitem"
          class="flex min-h-11 w-full items-center gap-2 rounded-md px-3 text-left text-sm hover:bg-accent md:min-h-9"
          onclick={() => {
            closeAccount();
            onLogout();
          }}
        >
          <LogOut size={16} class="shrink-0 text-muted-foreground" />
          Log out
        </button>
      </div>
    {/if}
  </div>
</header>
