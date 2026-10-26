import { getCurrentWebview } from "@tauri-apps/api/webview";
import { filesFromPaths } from "./native-paths";
import type { NativeDropEvent } from "./native-types";

/**
 * Subscribe to OS file drops on the Tauri webview.
 * Paths stay on disk; the Rust upload command streams them with progress.
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

    try {
      const batch = await filesFromPaths(payload.paths);
      handler({ type: "drop", batch });
    } catch (cause) {
      handler({
        type: "error",
        message: dropErrorMessage(cause),
      });
    }
  });
}

function dropErrorMessage(cause: unknown) {
  if (cause instanceof Error && cause.message) {
    return cause.message;
  }

  return "Could not read the dropped files";
}
