const units = ["B", "KB", "MB", "GB", "TB"] as const;

/** `0` is the server's unlimited quota. */
export function formatQuota(bytes: number): string {
  if (bytes <= 0) return "Unlimited";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const shown = unit === 0 || value >= 10 ? value.toFixed(0) : value.toFixed(1);
  return `${shown} ${units[unit]}`;
}

export function formatDatetime(iso: string): string {
  const date = new Date(iso);

  if (Number.isNaN(date.getTime())) return iso;

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
