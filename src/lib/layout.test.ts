import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('getSplitDirectionFromDrag', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn()
    });
  });

  it('keeps left-right split when dragging mostly horizontally', async () => {
    const { getSplitDirectionFromDrag } = await import('./layout');
    expect(
      getSplitDirectionFromDrag('vertical', { x: 100, y: 100 }, { x: 180, y: 120 })
    ).toBe('vertical');
  });

  it('switches left-right split to top-bottom when dragging strongly vertically', async () => {
    const { getSplitDirectionFromDrag } = await import('./layout');
    expect(
      getSplitDirectionFromDrag('vertical', { x: 100, y: 100 }, { x: 125, y: 180 })
    ).toBe('horizontal');
  });

  it('switches top-bottom split to left-right when dragging strongly horizontally', async () => {
    const { getSplitDirectionFromDrag } = await import('./layout');
    expect(
      getSplitDirectionFromDrag('horizontal', { x: 100, y: 100 }, { x: 180, y: 120 })
    ).toBe('vertical');
  });
});
