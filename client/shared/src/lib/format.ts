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
