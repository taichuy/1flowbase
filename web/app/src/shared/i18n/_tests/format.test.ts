import { describe, expect, test } from 'vitest';

import { formatTokenCount } from '../format';

describe('formatTokenCount', () => {
  test('uses decimal K, M, and B units with one decimal place', () => {
    expect(formatTokenCount(999)).toBe('999');
    expect(formatTokenCount(1_000)).toBe('1K');
    expect(formatTokenCount(16_262)).toBe('16.3K');
    expect(formatTokenCount(1_250_000)).toBe('1.3M');
    expect(formatTokenCount(1_000_000_000)).toBe('1B');
  });
});
