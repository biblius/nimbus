<script lang="ts">
  import type { Snippet } from "svelte";
  import type { ClassValue } from "svelte/elements";

  let {
    class: className,
    children,
  }: {
    class?: ClassValue;
    children: Snippet;
  } = $props();
</script>

<main class="auth-shell">
  <div class="auth-shell-glow" aria-hidden="true"></div>
  <div class={["auth-shell-inner", className]}>
    {@render children()}
  </div>
</main>

<style>
  .auth-shell {
    position: relative;
    display: flex;
    min-height: 100dvh;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    padding: 1.5rem;
  }

  .auth-shell-glow {
    pointer-events: none;
    position: absolute;
    inset: 0;
    background:
      radial-gradient(
        ellipse 70% 45% at 50% 0%,
        color-mix(in oklab, var(--primary) 22%, transparent),
        transparent 70%
      ),
      radial-gradient(
        ellipse 50% 40% at 85% 90%,
        color-mix(in oklab, var(--primary) 8%, transparent),
        transparent 60%
      ),
      var(--background);
  }

  .auth-shell-inner {
    position: relative;
    z-index: 1;
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: center;
  }

  .auth-shell :global(.auth-panel) {
    background: color-mix(in oklab, var(--card) 92%, transparent);
    box-shadow: 0 24px 48px color-mix(in oklab, #000 35%, transparent);
    backdrop-filter: blur(10px);
    animation: auth-panel-in 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes -global-auth-panel-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .auth-shell :global(.auth-panel) {
      animation: none;
    }
  }
</style>
