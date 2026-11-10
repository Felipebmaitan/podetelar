export function dataSize(bytes: number) {
  return bytes >= 1e9
    ? `${(bytes / 1e9).toLocaleString(undefined, { maximumFractionDigits: 2 })} GB`
    : `${(bytes / 1e6).toLocaleString(undefined, { maximumFractionDigits: 2 })} MB`;
}
export type Period =
  "Real time" | "Today" | "7 days" | "Month" | "Custom period";
export function periodRange(
  period: Period,
  now = new Date(),
): [number, number] {
  const end = Math.ceil(now.getTime() / 60000) * 60000;
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  if (period === "Real time") return [end - 15 * 60000, end];
  if (period === "7 days") start.setDate(start.getDate() - 6);
  if (period === "Month") start.setDate(1);
  return [start.getTime(), end];
}
export const speed = (bytes: number, duration: number) =>
  duration > 0 ? (bytes * 8) / duration / 1000 : 0;
