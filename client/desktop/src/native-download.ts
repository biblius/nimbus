import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { ErrorBody } from "@nimbus/shared/api-errors";

type DownloadHttpResponse = {
  status: number;
  body: ErrorBody | unknown | null;
};

type DownloadCommandError = {
  kind: "io" | "network";
  message?: string;
};

/**
 * Prompt for a save path, then stream the HTTP response to disk via Rust.
 * Returns `null` if the user cancels the dialog.
 */
export async function nativeDownloadToPath(
  url: string,
  token: string | null,
  name: string,
): Promise<DownloadHttpResponse | null> {
  const path = await save({ defaultPath: name });

  if (path === null) {
    return null;
  }

  try {
    return await invoke<DownloadHttpResponse>("download_to_path", {
      url,
      token,
      path,
    });
  } catch (cause) {
    if (isDownloadCommandError(cause) && cause.kind === "network") {
      throw new TypeError(cause.message ?? "network error");
    }

    if (isDownloadCommandError(cause) && cause.kind === "io") {
      throw new TypeError(cause.message ?? "io error");
    }

    throw cause;
  }
}

function isDownloadCommandError(value: unknown): value is DownloadCommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    (value.kind === "io" || value.kind === "network")
  );
}
