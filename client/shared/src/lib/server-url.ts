import { session } from "./session.svelte";

/**
 * Parse a Nimbus server origin. Must be `http(s)://host[:port]` with no path,
 * query, or hash.
 */
export function parseServerOrigin(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) {
    return null;
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return null;
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return null;
  }

  if (parsed.pathname !== "/" && parsed.pathname !== "") {
    return null;
  }

  if (parsed.search || parsed.hash) {
    return null;
  }

  return parsed.origin;
}

/** Public share page origin: configured server on native, page origin on web. */
export function publicShareOrigin(): string {
  if (session.native && session.endpoint) {
    return session.endpoint;
  }
  return window.location.origin;
}

/** Absolute public link for a shared file id. */
export function publicShareLink(fileId: string): string {
  return `${publicShareOrigin()}/files/shared/${encodeURIComponent(fileId)}`;
}
