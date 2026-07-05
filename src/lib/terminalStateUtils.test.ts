import { describe, expect, it } from 'vitest';

import {
  computeNextSelectedIndexAfterRemoval,
  computeSelectedIndexAfterBatchRemoval,
} from './terminalStateUtils';

const term = (id: string) => ({ sessionId: id });

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

describe('computeSelectedIndexAfterBatchRemoval', () => {
  it('keeps the selected session selected at its new position', () => {
    // [rootA(0), childA1(1), childA2(2), rootB(3)] selecting rootB, remove children of A
    const list = [term('rootA'), term('childA1'), term('childA2'), term('rootB')];
    const removed = new Set(['childA1', 'childA2']);
    // rootB survives at new index 1
    expect(computeSelectedIndexAfterBatchRemoval(list, 3, removed)).toBe(1);
  });

  it('does not go out of bounds after batch removal (disconnect repro)', () => {
    // [rootA(0), childA1(1), childA2(2), rootB(3)] selecting rootB(3),
    // removing rootA + its two children leaves only [rootB]
    const list = [term('rootA'), term('childA1'), term('childA2'), term('rootB')];
    const removed = new Set(['rootA', 'childA1', 'childA2']);
    const idx = computeSelectedIndexAfterBatchRemoval(list, 3, removed);
    expect(idx).toBe(0);
    expect(idx).toBeLessThan(list.length - removed.size + 1);
  });

  it('picks a nearby survivor when the selected session is removed (closePane repro)', () => {
    // [rootA(0), childA(1), rootB(2)] selecting index 2, close childA -> index must stay valid
    const list = [term('rootA'), term('childA'), term('rootB')];
    const removed = new Set(['childA']);
    // rootB survives, was index 2, now index 1
    expect(computeSelectedIndexAfterBatchRemoval(list, 2, removed)).toBe(1);
  });

  it('picks a nearby survivor when the selected session itself is removed', () => {
    const list = [term('a'), term('b'), term('c')];
    // selecting b (index 1), remove b -> should land on a survivor near old position
    const removed = new Set(['b']);
    const idx = computeSelectedIndexAfterBatchRemoval(list, 1, removed);
    expect([0, 1]).toContain(idx);
    expect(idx).toBeLessThanOrEqual(1);
  });

  it('returns 0 when everything is removed', () => {
    const list = [term('a'), term('b')];
    expect(computeSelectedIndexAfterBatchRemoval(list, 1, new Set(['a', 'b']))).toBe(0);
  });

  it('handles an invalid/stale currentIndex without throwing', () => {
    const list = [term('a'), term('b')];
    // currentIndex 5 is out of range; b survives
    expect(computeSelectedIndexAfterBatchRemoval(list, 5, new Set(['a']))).toBe(0);
  });
});
