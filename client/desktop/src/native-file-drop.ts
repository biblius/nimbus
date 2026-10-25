import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { NativeDropEvent } from "@nimbus/shared/session";
import { filesFromPaths } from "./native-paths";

/**
 * Subscribe to OS file drops on the Tauri webview.
 * Paths are read into `File`s so the shared upload pipeline can reuse them.
 */
export async function listenNativeFileDrop(
  handler: (event: NativeDropEvent) => void,
): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent(async (event) => {
    const payload = event.payload;

    if (payload.type === "enter") {
      handler({ type: "enter" });
      return;
    }

    if (payload.type === "leave") {
      handler({ type: "leave" });
      return;
    }

    if (payload.type !== "drop") {
      return;
    }

    const batch = await filesFromPaths(payload.paths);
    handler({ type: "drop", batch });
  });
}
