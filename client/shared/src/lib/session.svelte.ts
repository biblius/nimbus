import { NimbusClient } from "./api";
import { navigate } from "./router";
import type { UserViewFull } from "./types";

/**
 * Current session state.
 */
export const session = $state<{
  client: NimbusClient;
  user: UserViewFull | null;
  menuOpen: boolean;
  /** Native (Tauri) client — login creates a permanent session. */
  native: boolean;
  /** Native only: persisted server URL, or `null` if not configured yet. */
  endpoint: string | null;
  /** Native only: persist a new server URL and rebuild the API client. */
  configureEndpoint: ((url: string) => Promise<void>) | null;
  /** Native only: open a URL in the system browser. */
  openExternalUrl: ((url: string) => Promise<void>) | null;
}>({
  /**
   * The Nimbus server API.
   * Set by the app entrypoint before the router mounts.
   */
  client: undefined!,

  /**
   * The current user, set on initial page load in SessionApp.
   */
  user: null,

  /**
   * Whether the sidebar is open (for small screens).
   */
  menuOpen: false,

  native: false,

  endpoint: null,

  configureEndpoint: null,

  openExternalUrl: null,
});

/** Clear local session state and send the user to the login page. */
export function clearSession() {
  session.user = null;
  session.menuOpen = false;
  navigate("/login", { replace: true });
}

export async function logout() {
  try {
    await session.client.logout();
  } finally {
    clearSession();
  }
}
