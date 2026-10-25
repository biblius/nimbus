import { invoke } from "@tauri-apps/api/core";

/** Write the HttpOnly session cookie into the webview jar for media streams. */
export async function setSessionCookie(
  origin: string,
  sessionId: string,
): Promise<void> {
  await invoke("set_session_cookie", {
    origin,
    value: sessionId,
  });
}

/** Remove the session cookie from the webview jar. */
export async function clearSessionCookie(origin: string): Promise<void> {
  await invoke("clear_session_cookie", { origin });
}
