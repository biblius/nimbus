const units = ["B", "KB", "MB", "GB", "TB"] as const;

/** Returns a user-friendly display string for the bytes.
 * The values are actually *iB, but we display them as their
 * metric equivalents so we don't confuse users.
 */
export function formatBytes(bytes: number): string {
  let value = Math.max(bytes, 0);

  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value.toFixed(1)} ${units[unit]}`;
}

/** `0` is the server's unlimited quota. */
export function formatQuota(bytes: number): string {
  if (bytes <= 0) {
    return "Unlimited";
  }
  return formatBytes(bytes);
}

export function formatDatetime(iso: string): string {
  const date = new Date(iso);

  if (Number.isNaN(date.getTime())) {
    return iso;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

const ELLIPSIS = "…";

let measureCanvas: HTMLCanvasElement | null = null;

/** Middle-truncate `value` so it fits `maxWidth` px in `font` (CSS font shorthand). */
export function fitMiddleEllipsis(
  value: string,
  maxWidth: number,
  font: string,
): string {
  if (maxWidth <= 0 || value.length === 0) {
    return value;
  }

  measureCanvas ??= document.createElement("canvas");
  const ctx = measureCanvas.getContext("2d");

  if (ctx == null) {
    return value;
  }

  ctx.font = font;

  if (ctx.measureText(value).width <= maxWidth) {
    return value;
  }

  if (ctx.measureText(ELLIPSIS).width >= maxWidth) {
    return ELLIPSIS;
  }

  let lo = 0;
  let hi = value.length;

  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    const headLen = Math.ceil(mid / 2);
    const tailLen = Math.floor(mid / 2);
    const candidate =
      value.slice(0, headLen) + ELLIPSIS + value.slice(value.length - tailLen);

    if (ctx.measureText(candidate).width <= maxWidth) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }

  if (lo <= 0) {
    return ELLIPSIS;
  }

  const headLen = Math.ceil(lo / 2);
  const tailLen = Math.floor(lo / 2);

  return (
    value.slice(0, headLen) + ELLIPSIS + value.slice(value.length - tailLen)
  );
}
