import type { DeletionEvent } from "./types";

/** Live progress for an admin hard-delete. Shown until the SSE stream ends. */
export const deletionBanner = $state({
  open: false,
  label: "Deleting…",
  percent: 0,
  failed: false,
});

/**
 * Drive the banner from a deletion SSE channel until `{ type: "end" }`.
 * Re-throws after the stream ends if any item failed (or the request failed).
 */
export async function trackDeletion(
  stream: AsyncGenerator<DeletionEvent>,
): Promise<void> {
  deletionBanner.open = true;
  deletionBanner.label = "Deleting…";
  deletionBanner.percent = 0;
  deletionBanner.failed = false;

  try {
    for await (const event of stream) {
      if (event.type === "progress") {
        const { result } = event;

        if (result.name) {
          deletionBanner.label = result.success
            ? `Deleted ${result.name}`
            : `Failed ${result.name}`;
        }

        if (!result.success) {
          deletionBanner.failed = true;
        }

        deletionBanner.percent = Math.min(90, deletionBanner.percent + 8);
      } else {
        deletionBanner.percent = 100;
        if (!deletionBanner.failed && deletionBanner.label === "Deleting…") {
          deletionBanner.label = "Deletion complete";
        }
      }
    }
  } catch (error) {
    deletionBanner.failed = true;
    deletionBanner.percent = 100;
    throw error;
  } finally {
    await new Promise((resolve) => setTimeout(resolve, 600));
    deletionBanner.open = false;
  }
}
