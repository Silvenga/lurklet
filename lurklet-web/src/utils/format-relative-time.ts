const RELATIVE_FORMAT = new Intl.RelativeTimeFormat(undefined, { numeric: "always" });
const DAY = 24 * 60 * 60 * 1000;
const UNITS = [
  ["year", 365 * DAY],
  ["month", 30 * DAY],
  ["day", DAY],
  ["hour", 60 * 60 * 1000],
  ["minute", 60 * 1000],
] as const;

export function formatRelativeTime(date: number, now: number) {
  const difference = date - now;
  const unit = UNITS.find(([, duration]) => Math.abs(difference) >= duration);
  return unit ? RELATIVE_FORMAT.format(Math.trunc(difference / unit[1]), unit[0]) : "just now";
}
