import { describe, expect, it } from 'vitest';

import { computeNextSelectedIndexAfterRemoval } from './terminalStateUtils';

describe('terminalStateUtils', () => {
  it('shifts selected index left when removing a tab before it', () => {
    expect(computeNextSelectedIndexAfterRemoval(1, 0, 3)).toBe(0);
    expect(computeNextSelectedIndexAfterRemoval(2, 0, 4)).toBe(1);
  });

  it('moves selection to a valid neighbor when removing selected tab', () => {
    expect(computeNextSelectedIndexAfterRemoval(1, 1, 3)).toBe(1);
    expect(computeNextSelectedIndexAfterRemoval(2, 2, 3)).toBe(1);
  });

  it('keeps selection when removing a tab after it', () => {
    expect(computeNextSelectedIndexAfterRemoval(0, 2, 3)).toBe(0);
  });

  it('falls back to zero when list becomes empty', () => {
    expect(computeNextSelectedIndexAfterRemoval(0, 0, 1)).toBe(0);
  });
});
