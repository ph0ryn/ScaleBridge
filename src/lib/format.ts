const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "medium",
});

export function formatDateTime(value: string | null | undefined): string {
  if (!value) {
    return "--";
  }

  const date = new Date(value);

  if (Number.isNaN(date.valueOf())) {
    return value;
  }

  return dateTimeFormat.format(date);
}

export function formatElapsed(value: string | null | undefined): string {
  if (!value) {
    return "--";
  }

  const timestamp = new Date(value).valueOf();

  if (Number.isNaN(timestamp)) {
    return "--";
  }

  const elapsedSeconds = Math.max(0, Math.round((Date.now() - timestamp) / 1000));

  if (elapsedSeconds < 60) {
    return `${elapsedSeconds}s ago`;
  }

  const elapsedMinutes = Math.round(elapsedSeconds / 60);

  if (elapsedMinutes < 60) {
    return `${elapsedMinutes}m ago`;
  }

  const elapsedHours = Math.round(elapsedMinutes / 60);

  return `${elapsedHours}h ago`;
}

export function formatWeight(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "--";
  }

  return `${value.toFixed(1)} kg`;
}

export function formatImpedance(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "--";
  }

  return `${value} ohm`;
}

export function formatStatusLabel(status: string): string {
  return status
    .split("_")
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

export function formatDeviceName(name: string | null, address: string | null): string {
  if (name && name.trim().length > 0) {
    return name;
  }

  return address ?? "Unknown device";
}

export function formatCount(value: number, noun: string): string {
  let suffix = "s";

  if (value === 1) {
    suffix = "";
  }

  return `${value} ${noun}${suffix}`;
}

export function truncateMiddle(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }

  const segmentLength = Math.floor((maxLength - 3) / 2);

  return `${value.slice(0, segmentLength)}...${value.slice(-segmentLength)}`;
}
