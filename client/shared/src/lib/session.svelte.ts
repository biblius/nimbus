import type { NimbusClient } from "./api";
import { navigate } from "./router";
import type { UserView } from "./types";

/**
 * Current session state.
 */
export const session = $state<{
  client: NimbusClient;
  user: UserView | null;
  menuOpen: boolean;
  /** Native (Tauri) client — login creates a permanent session. */
  native: boolean;
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
});

export async function logout() {
  await session.client.logout();
  session.user = null;
  session.menuOpen = false;
  navigate("/login", { replace: true });
}
