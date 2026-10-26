import type { NimbusApi } from "./nimbus-api";
import type { UploadHandlerOptions } from "./pending-upload";
import { navigate } from "./router";
import type { UserView } from "./types";

/**
 * Current session state.
 *
 * Platform upload handlers are created via `createUploadHandler` (set by each
 * app). Native-only hooks stay loosely typed where the desktop app wires them.
 */
export const session = $state<{
  client: NimbusApi;
  user: UserView | null;
  menuOpen: boolean;
  /** Native (Tauri) client - login creates a permanent session. */
  native: boolean;
  /** Native only: persisted server origin (no path), or `null` if unset. */
  endpoint: string | null;
  /** Native only: persist a bare server origin and rebuild the API client. */
  configureEndpoint: ((url: string) => Promise<void>) | null;
  /** Native only: open a URL in the system browser. */
  openExternalUrl: ((url: string) => Promise<void>) | null;
  /**
   * Platform upload queue (web File/XHR vs desktop path streaming).
   * Set by the app entrypoint; returns the concrete handler for FileBrowser.
   */
  createUploadHandler: ((opts: UploadHandlerOptions) => any) | null;
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

  createUploadHandler: null,
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
