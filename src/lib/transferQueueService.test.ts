import { describe, expect, it } from 'vitest';

import { formatTransferRate } from './transferRateFormatter';

describe('formatTransferRate', () => {
  it('returns zero for invalid or non-positive values', () => {
    expect(formatTransferRate(0)).toBe('0 B/s');
    expect(formatTransferRate(-1)).toBe('0 B/s');
    expect(formatTransferRate(Number.NaN)).toBe('0 B/s');
    expect(formatTransferRate(Number.POSITIVE_INFINITY)).toBe('0 B/s');
  });

  it('formats bytes per second without unit promotion', () => {
    expect(formatTransferRate(1)).toBe('1 B/s');
    expect(formatTransferRate(512.4)).toBe('512.4 B/s');
    expect(formatTransferRate(1023)).toBe('1023 B/s');
  });

  it('formats kilobytes and megabytes', () => {
    expect(formatTransferRate(1024)).toBe('1 KB/s');
    expect(formatTransferRate(1536)).toBe('1.5 KB/s');
    expect(formatTransferRate(1024 * 1024)).toBe('1 MB/s');
    expect(formatTransferRate(1024 * 1024 * 3.25)).toBe('3.25 MB/s');
  });

  it('formats gigabytes and terabytes', () => {
    expect(formatTransferRate(1024 ** 3)).toBe('1 GB/s');
    expect(formatTransferRate(1024 ** 4)).toBe('1 TB/s');
  });
});
