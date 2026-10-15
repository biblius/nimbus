import type { NimbusClient } from "./api";
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
});

export async function logout() {
  await session.client.logout();
  session.user = null;
  session.menuOpen = false;
  navigate("/login", { replace: true });
}
