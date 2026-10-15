<script lang="ts">
  import {
    ChevronDown,
    Cloud,
    LogOut,
    Moon,
    Settings,
    Sun,
  } from "@lucide/svelte";
  import { navigate } from "../router";
  import { theme, toggleTheme } from "../theme.svelte";
  import type { UserView } from "../types";
  import Button from "./ui/button/button.svelte";
  import MenuItem from "./ui/menu/menu-item.svelte";
  import MenuSeparator from "./ui/menu/menu-separator.svelte";
  import Menu from "./ui/menu/menu.svelte";

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
</script>

<header
  class="sticky top-0 z-50 flex h-14 items-center gap-3 border-b bg-card px-3 sm:px-5"
>
  <Button
    type="button"
    variant="ghost"
    size="icon"
    class="size-10 text-muted-foreground hover:text-foreground md:hidden"
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
  </Button>

  <div class="flex min-w-0 items-center gap-2.5">
    <img src="/favicon.png" alt="" class="size-7 shrink-0" />
    <p class="truncate text-[15px] font-semibold tracking-tight">Nimbus</p>
  </div>

  <div class="ml-auto">
    <Menu class="mt-2 w-56">
      {#snippet trigger(props, open)}
        <button
          type="button"
          class={[
            "flex h-10 max-w-[min(16rem,calc(100vw-8.5rem))] items-center gap-2.5 rounded-md border border-transparent py-1 pr-2 pl-1 transition-colors hover:border-input hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
            open && "border-input bg-accent",
          ]}
          aria-label="Open account menu"
          {...props}
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
            <span
              class="block truncate text-xs text-muted-foreground capitalize"
            >
              {user.role}
            </span>
          </span>
          <ChevronDown
            size={14}
            class={[
              "shrink-0 text-muted-foreground transition-transform",
              open && "rotate-180",
            ]}
          />
        </button>
      {/snippet}

      {#if admin}
        <MenuItem
          aria-label={mode === "user"
            ? "Switch to the admin panel"
            : "Switch to your files"}
          onclick={onToggleMode}
        >
          <Cloud size={16} />
          {mode === "user" ? "Admin panel" : "User panel"}
        </MenuItem>
      {/if}

      <MenuItem onclick={() => navigate("/settings")}>
        <Settings size={16} />
        Account
      </MenuItem>

      <MenuItem closeOnSelect={false} onclick={() => toggleTheme()}>
        {#if theme.mode === "dark"}
          <Sun size={16} />
          Light mode
        {:else}
          <Moon size={16} />
          Dark mode
        {/if}
      </MenuItem>

      <MenuSeparator />

      <MenuItem onclick={onLogout}>
        <LogOut size={16} />
        Log out
      </MenuItem>
    </Menu>
  </div>
</header>
