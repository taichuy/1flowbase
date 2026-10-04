import { describe, expect, it, vi } from 'vitest';
import { RetentionStats } from '../retention-stats';

function storageFixture() {
  const values = new Map<string, string>();
  return {
    values,
    storage: {
      getItem: vi.fn((key: string) => values.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => {
        values.set(key, value);
      })
    }
  };
}

describe('bounded retention frequency metadata', () => {
  it('persists only numeric sketch statistics and isolates navigation scopes', () => {
    const { storage, values } = storageFixture();
    const stats = new RetentionStats({ scope: 'actor-a/workspace-a', storage });
    const key = '/private/page?draft=secret&csrf=secret';
    stats.record(key);
    stats.record(key);
    stats.flush();
    expect(
      new RetentionStats({ scope: 'actor-a/workspace-a', storage }).frequency(
        key
      )
    ).toBe(2);
    expect(
      new RetentionStats({ scope: 'actor-b/workspace-a', storage }).frequency(
        key
      )
    ).toBe(0);
    expect(
      new RetentionStats({ scope: 'actor-a/workspace-b', storage }).frequency(
        key
      )
    ).toBe(0);
    expect([...values.keys()].join('')).not.toContain('actor-a');
    const payload = [...values.values()][0];
    expect(payload).not.toContain('private');
    expect(payload).not.toContain('draft');
    expect(payload).not.toContain('secret');
    expect(payload).not.toContain('csrf');
    expect(Object.keys(JSON.parse(payload)).sort()).toEqual([
      'accesses',
      'counters',
      'seen',
      'version',
      'width'
    ]);
  });

  it('ages historical popularity by accesses without a time expiry', () => {
    const stats = new RetentionStats({
      scope: 'scope',
      storage: null,
      byteBudget: 512
    });
    for (let index = 0; index < 12; index += 1) stats.record('old-hot');
    const previous = stats.frequency('old-hot');
    for (let index = 0; index < 2000; index += 1) stats.record('new-hot');
    expect(previous).toBeGreaterThan(0);
    expect(stats.frequency('old-hot')).toBeLessThan(previous);
    expect(stats.frequency('new-hot')).toBeGreaterThan(
      stats.frequency('old-hot')
    );
  });

  it('ages a former hot page within a normal B/C working-set shift', () => {
    const stats = new RetentionStats({ scope: 'scope', storage: null });
    for (let index = 0; index < 12; index += 1) stats.record('a');
    for (let index = 0; index < 80; index += 1)
      stats.record(index % 2 ? 'b' : 'c');
    expect(stats.frequency('a')).toBeLessThanOrEqual(1);
    expect(stats.frequency('b')).toBeGreaterThan(stats.frequency('a'));
    expect(stats.frequency('c')).toBeGreaterThan(stats.frequency('a'));
  });

  it('restores adaptive era progress and rejects impossible era counters', () => {
    const { storage, values } = storageFixture();
    const initial = new RetentionStats({ scope: 'scope', storage });
    for (let index = 0; index < 19; index += 1) initial.record('page');
    initial.flush();
    const restored = new RetentionStats({ scope: 'scope', storage });
    restored.record('page');
    expect(restored.frequency('page')).toBeLessThan(15);
    const [key, value] = [...values][0];
    const data = JSON.parse(value);
    data.accesses = Number.MAX_SAFE_INTEGER;
    values.set(key, JSON.stringify(data));
    expect(
      new RetentionStats({ scope: 'scope', storage }).frequency('page')
    ).toBe(0);
  });

  it('bounds persistence independently of the number of distinct page accesses', () => {
    const { storage, values } = storageFixture();
    const stats = new RetentionStats({
      scope: 'scope',
      storage,
      byteBudget: 512
    });
    for (let index = 0; index < 20000; index += 1)
      stats.record(`page-${index}`);
    stats.flush();
    expect(values.size).toBe(1);
    expect([...values.values()][0].length).toBeLessThanOrEqual(512);
    const data = JSON.parse([...values.values()][0]);
    expect(
      data.counters.every(
        (count: number) => Number.isInteger(count) && count >= 0 && count <= 15
      )
    ).toBe(true);
  });

  it.each([
    'not-json',
    '{"version":1,"width":4096,"accesses":0,"counters":[]}',
    '{"version":2,"width":1,"accesses":0,"counters":[1,1,1,1]}',
    'x'.repeat(513)
  ])('ignores a corrupt or oversized record', (payload) => {
    const storage = { getItem: () => payload, setItem: vi.fn() };
    const stats = new RetentionStats({
      scope: 'scope',
      storage,
      byteBudget: 512
    });
    expect(stats.frequency('page')).toBe(0);
    stats.record('page');
    expect(stats.frequency('page')).toBe(1);
  });

  it('ignores invalid counters even when dimensions and version match', () => {
    const { storage, values } = storageFixture();
    const original = new RetentionStats({
      scope: 'scope',
      storage,
      byteBudget: 512
    });
    original.record('page');
    original.flush();
    const [key, value] = [...values][0];
    const data = JSON.parse(value);
    data.counters[0] = -1;
    values.set(key, JSON.stringify(data));
    expect(
      new RetentionStats({
        scope: 'scope',
        storage,
        byteBudget: 512
      }).frequency('page')
    ).toBe(0);
  });

  it('continues in memory when access or quota fails or storage is absent', () => {
    const storage = {
      getItem: () => {
        throw new Error('denied');
      },
      setItem: () => {
        throw new Error('quota');
      }
    };
    for (const candidate of [storage, null]) {
      const stats = new RetentionStats({ scope: 'scope', storage: candidate });
      stats.record('page');
      expect(() => stats.flush()).not.toThrow();
      expect(stats.frequency('page')).toBe(1);
    }
  });

  it('does not persist when the caller byte allowance cannot hold the header', () => {
    const { storage } = storageFixture();
    const stats = new RetentionStats({
      scope: 'scope',
      storage,
      byteBudget: 1
    });
    stats.record('page');
    stats.flush();
    expect(storage.setItem).not.toHaveBeenCalled();
  });
});
