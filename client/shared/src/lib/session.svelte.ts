import type { NimbusClient } from "./api";
import { navigate } from "./router";
import type { UserView } from "./types";

/**
 * Current session state.
 */
export const session = $state<{
  /**
   * The Nimbus server API.
   */
  client: NimbusClient;
  /**
   * The current user.
   */
  user: UserView | null;
  /**
   * Whether the sidebar is open (for small screens).
   */
  menuOpen: boolean;
}>({
  // Set by the app entrypoint before the router mounts.
  client: undefined!,
  user: null,
  menuOpen: false,
});

export async function logout() {
  await session.client.logout();
  session.user = null;
  session.menuOpen = false;
  navigate("/login", { replace: true });
}
