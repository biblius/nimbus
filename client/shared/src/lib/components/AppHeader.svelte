<script lang="ts">
  import { ChevronDown, LogOut, Moon, Sun } from "@lucide/svelte";
  import { theme, toggleTheme } from "../theme.svelte";
  import type { UserView } from "../types";

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
  const initial = $derived(user.username.charAt(0).toUpperCase());

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
  class="sticky top-0 z-50 flex h-14 items-center gap-3 border-b bg-card px-3 sm:px-5"
>
  <button
    type="button"
    class="inline-flex size-10 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground md:hidden"
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

  <div class="flex min-w-0 items-center gap-2.5">
    <img src="/favicon.png" alt="" class="size-7 shrink-0" />
    <p class="truncate text-[15px] font-semibold tracking-tight">Nimbus</p>
  </div>

  <div class="relative ml-auto" bind:this={accountMenu}>
    <button
      type="button"
      class={[
        "flex h-10 max-w-[min(16rem,calc(100vw-8.5rem))] items-center gap-2.5 rounded-md border border-transparent py-1 pr-2 pl-1 transition-colors hover:border-input hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
        accountOpen && "border-input bg-accent",
      ]}
      aria-label="Open account menu"
      aria-haspopup="menu"
      aria-expanded={accountOpen}
      onclick={(event) => {
        event.stopPropagation();
        accountOpen = !accountOpen;
      }}
    >
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-md border border-input bg-muted text-sm font-semibold text-foreground"
        aria-hidden="true"
      >
        {initial}
      </span>
      <span class="min-w-0 text-left leading-tight">
        <span class="block truncate text-sm font-medium text-foreground">
          {user.username}
        </span>
        <span class="block truncate text-xs text-muted-foreground capitalize">
          {user.role}
        </span>
      </span>
      <ChevronDown
        size={14}
        class="shrink-0 text-muted-foreground transition-transform {accountOpen
          ? 'rotate-180'
          : ''}"
      />
    </button>

    {#if accountOpen}
      <div
        role="menu"
        class="absolute top-full right-0 z-50 mt-2 w-56 overflow-hidden rounded-xl border bg-card shadow-lg"
      >
        <div class="border-b px-3 py-2.5">
          <p class="truncate text-sm font-medium">{user.username}</p>
          <p class="truncate text-xs text-muted-foreground capitalize">
            {user.role}
          </p>
        </div>
        <div class="p-1">
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
      </div>
    {/if}
  </div>
</header>
