import { Channel, invoke } from "@tauri-apps/api/core";
import {
  parseError,
  type ErrorBody,
  type UploadProgress,
} from "@nimbus/shared/api-errors";
import type { NativeUploadFile } from "./native-types";

type UploadProgressEvent = {
  loaded: number;
  total: number;
};

type UploadHttpResponse = {
  status: number;
  body: ErrorBody | unknown | null;
};

type UploadCommandError = {
  kind: "aborted" | "io" | "network";
  message?: string;
};

let nextUploadId = 1;

/**
 * Multipart upload via the Rust `upload_multipart` command.
 * Streams from disk and reports byte progress over a Channel.
 */
export async function nativeUploadMultipart(
  url: string,
  token: string,
  parent: string | null,
  files: NativeUploadFile[],
  onProgress?: UploadProgress,
  signal?: AbortSignal,
): Promise<UploadHttpResponse> {
  if (signal?.aborted) {
    throw new DOMException("upload aborted", "AbortError");
  }

  const uploadId = nextUploadId++;
  const onProgressChannel = new Channel<UploadProgressEvent>();

  onProgressChannel.onmessage = (event) => {
    onProgress?.(event.loaded, event.total);
  };

  const onAbort = () => {
    void invoke("cancel_upload", { uploadId }).catch(() => {});
  };

  signal?.addEventListener("abort", onAbort);

  try {
    return await invoke<UploadHttpResponse>("upload_multipart", {
      uploadId,
      url,
      token,
      parent,
      files: files.map((file) => ({
        path: file.path,
        filename: file.name,
      })),
      onProgress: onProgressChannel,
    });
  } catch (cause) {
    if (isUploadCommandError(cause) && cause.kind === "aborted") {
      throw new DOMException("upload aborted", "AbortError");
    }

    if (signal?.aborted) {
      throw new DOMException("upload aborted", "AbortError");
    }

    if (isUploadCommandError(cause) && cause.kind === "network") {
      throw new TypeError(cause.message ?? "network error");
    }

    if (isUploadCommandError(cause) && cause.kind === "io") {
      throw new TypeError(cause.message ?? "io error");
    }

    throw cause;
  } finally {
    signal?.removeEventListener("abort", onAbort);
  }
}

export function nativeUploadHttpError(
  status: number,
  body: unknown,
  onUnauthorized?: () => void,
): Error {
  const payload =
    body && typeof body === "object" ? (body as ErrorBody) : null;

  if (status === 401) {
    onUnauthorized?.();
  }

  return parseError(status, payload);
}

function isUploadCommandError(value: unknown): value is UploadCommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    (value.kind === "aborted" ||
      value.kind === "io" ||
      value.kind === "network")
  );
}
