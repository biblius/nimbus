import type { NimbusApi } from "./nimbus-api";
import { navigate } from "./router";
import type { UserView } from "./types";

/** Files collected from a native (Tauri) OS drop. */
export type NativeDropBatch = {
  files: File[];
  /** Each inner list is one dropped folder tree (relative paths in `name`). */
  directories: File[][];
};

/** OS drag-drop lifecycle from the Tauri webview. */
export type NativeDropEvent =
  | { type: "enter" }
  | { type: "leave" }
  | { type: "drop"; batch: NativeDropBatch };

/**
 * Current session state.
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
   * Native only: listen for OS file drops on the window.
   * HTML5 drag-drop does not fire inside Tauri while native DnD is enabled.
   */
  listenFileDrop:
    | ((handler: (event: NativeDropEvent) => void) => Promise<() => void>)
    | null;
  /** Native only: OS multi-file open dialog. `null` if cancelled. */
  pickFiles: (() => Promise<File[] | null>) | null;
  /**
   * Native only: OS folder open dialog.
   * Returns path-prefixed files for `uploadDir`, or `null` if cancelled.
   */
  pickFolder: (() => Promise<File[] | null>) | null;
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

  listenFileDrop: null,

  pickFiles: null,

  pickFolder: null,
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
