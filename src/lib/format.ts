const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "medium",
});

export function formatDateTime(value: unknown): string {
  const date = parseDateTime(value);

  if (!date) {
    return "--";
  }

  return dateTimeFormat.format(date);
}

export function formatElapsed(value: unknown): string {
  const date = parseDateTime(value);

  if (!date) {
    return "--";
  }

  const timestamp = date.valueOf();

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

function parseDateTime(value: unknown): Date | null {
  if (!value) {
    return null;
  }

  if (value instanceof Date) {
    return validDateOrNull(value);
  }

  if (Array.isArray(value)) {
    return parseOffsetDateTimeTuple(value);
  }

  if (typeof value !== "string") {
    return null;
  }

  const date = validDateOrNull(new Date(value));

  if (date) {
    return date;
  }

  return parseOffsetDateTimeTuple(value.split(","));
}

function parseOffsetDateTimeTuple(parts: unknown[]): Date | null {
  if (parts.length !== 9) {
    return null;
  }

  const numbers = parts.map((part) => Number(part));

  if (numbers.some((part) => !Number.isFinite(part))) {
    return null;
  }

  const [year, ordinal, hour, minute, second, nanosecond, offsetHour, offsetMinute, offsetSecond] =
    numbers;
  const utcMilliseconds =
    Date.UTC(year, 0, ordinal, hour, minute, second, Math.floor(nanosecond / 1_000_000)) -
    ((offsetHour * 60 + offsetMinute) * 60 + offsetSecond) * 1000;

  return validDateOrNull(new Date(utcMilliseconds));
}

function validDateOrNull(date: Date): Date | null {
  if (Number.isNaN(date.valueOf())) {
    return null;
  }

  return date;
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
