// The queue implementation was removed because it was not connected to the
// production transfer path. TerminalManager only depends on this formatter.
export function formatSpeed(bytesPerSecond: number): string {
  if (!Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) return '0 B/s';

  const base = 1024;
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s', 'TB/s'];
  let value = bytesPerSecond;
  let unitIndex = 0;

  while (value >= base && unitIndex < units.length - 1) {
    value /= base;
    unitIndex += 1;
  }

  return `${parseFloat(value.toFixed(2))} ${units[unitIndex]}`;
}
