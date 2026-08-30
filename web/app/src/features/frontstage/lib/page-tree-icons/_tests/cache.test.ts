import { describe, expect, it, vi } from 'vitest';

import { IconComponentCache } from '../cache';

describe('IconComponentCache', () => {
  it('DV-F03 creates one identity for repeated icon demand', () => {
    const cache = new IconComponentCache<object>(2);
    const factory = vi.fn(() => ({}));

    const first = cache.getOrCreate('A', factory);
    const second = cache.getOrCreate('A', factory);

    expect(second).toBe(first);
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('DV-F03 evicts the least recently used icon at the bound', () => {
    const cache = new IconComponentCache<object>(2);
    cache.getOrCreate('A', () => ({}));
    cache.getOrCreate('B', () => ({}));
    cache.getOrCreate('A', () => ({}));
    cache.getOrCreate('C', () => ({}));

    expect(cache.size).toBe(2);
    expect(cache.has('A')).toBe(true);
    expect(cache.has('B')).toBe(false);
    expect(cache.has('C')).toBe(true);
  });
});
