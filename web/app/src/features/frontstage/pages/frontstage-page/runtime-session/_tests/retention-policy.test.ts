import { describe, expect, it } from 'vitest';
import { RetentionPolicy, type RetentionCandidate } from '../retention-policy';
import { RetentionStats } from '../retention-stats';

function fixture(byteBudget?: number) {
  const stats = new RetentionStats({
    scope: 'scope',
    storage: null,
    byteBudget
  });
  const policy = new RetentionPolicy(stats);
  const candidates = new Map<string, RetentionCandidate>();
  const activate = (
    key: string,
    cost: number,
    budget: number,
    protectedPage = false
  ) => {
    policy.activate(key);
    candidates.set(key, { key, cost, protected: protectedPage });
    const evictions = policy.reconcile([...candidates.values()], budget);
    for (const evicted of evictions) candidates.delete(evicted);
    return evictions;
  };
  return { stats, policy, candidates, activate };
}

describe('weighted W-TinyLFU page retention', () => {
  it('resists one-hit scans while retaining the newest inactive window page', () => {
    const { stats, candidates, activate } = fixture();
    for (let index = 0; index < 12; index += 1) stats.record('hot');
    activate('hot', 1, 4);
    for (let index = 0; index < 3; index += 1) activate(`seed-${index}`, 1, 4);
    activate('hot', 1, 4);
    for (let index = 0; index < 100; index += 1) {
      activate(`scan-${index}`, 1, 4);
      expect(candidates.has('hot')).toBe(true);
      expect(candidates.has(`scan-${index}`)).toBe(true);
      expect(
        [...candidates.values()].reduce((total, page) => total + page.cost, 0)
      ).toBeLessThanOrEqual(4);
    }
  });

  it('allows A/B/A with two large pages when the passed budget fits both', () => {
    const { candidates, activate } = fixture();
    expect(activate('a', 60, 120)).toEqual([]);
    expect(activate('b', 60, 120)).toEqual([]);
    expect(activate('a', 60, 120)).toEqual([]);
    expect([...candidates.keys()]).toEqual(['a', 'b']);
  });

  it('uses recency to break equal-frequency admission ties', () => {
    const { candidates, activate } = fixture();
    activate('old', 1, 2);
    activate('middle', 1, 2);
    const evictions = activate('latest', 1, 2);
    expect(evictions).toEqual(['old']);
    expect([...candidates.keys()]).toEqual(['middle', 'latest']);
  });

  it('frequency ranking remains independent of victim size', () => {
    const { stats, candidates, activate } = fixture();
    for (let index = 0; index < 4; index += 1) stats.record('large-hot');
    activate('large-hot', 6, 8);
    activate('small-cold', 1, 8);
    for (let index = 0; index < 2; index += 1) stats.record('small-challenger');
    activate('small-challenger', 1, 8);
    const evictions = activate('latest', 1, 8);
    // The challenger is cheaper but its lower hit count cannot defeat the hot
    // victim. A frequency/cost implementation would make the opposite choice.
    expect(evictions).toContain('small-challenger');
    expect(candidates.has('large-hot')).toBe(true);
  });

  it.each([false, true])(
    'compares all size victims before admission (second victim hot=%s)',
    (secondHot) => {
      const { stats, candidates, activate } = fixture();
      activate('first-victim', 2, 8);
      if (secondHot)
        for (let index = 0; index < 10; index += 1)
          stats.record('second-victim');
      activate('second-victim', 2, 8);
      activate('seed-window', 1, 8);
      for (let index = 0; index < 5; index += 1) stats.record('incoming');
      activate('incoming', 3, 8);
      const evictions = activate('latest', 3, 8);
      if (secondHot) {
        expect(evictions).toEqual(['incoming']);
        expect(candidates.has('first-victim')).toBe(true);
        expect(candidates.has('second-victim')).toBe(true);
      } else {
        expect(evictions).toEqual(['first-victim', 'second-victim']);
        expect(candidates.has('incoming')).toBe(true);
      }
    }
  );

  it('rejects an oversized page before it can displace fitting residents', () => {
    const { candidates, activate } = fixture();
    activate('a', 4, 10);
    activate('b', 6, 10);
    expect(activate('oversized', 11, 10)).toEqual(['oversized']);
    expect([...candidates.keys()]).toEqual(['a', 'b']);
  });

  it('treats protected costs as reserved and never evicts protected candidates', () => {
    const { policy } = fixture();
    for (const key of ['protected', 'old', 'new']) policy.activate(key);
    const evictions = policy.reconcile(
      [
        { key: 'protected', cost: 5, protected: true },
        { key: 'old', cost: 4, protected: false },
        { key: 'new', cost: 4, protected: false }
      ],
      9
    );
    expect(evictions).toEqual(['old']);
    expect(
      policy.reconcile([{ key: 'protected', cost: 5, protected: true }], 0)
    ).toEqual([]);
  });

  it('returns only eligible candidates, never the current or removed key', () => {
    const { policy } = fixture();
    for (const key of ['current', 'removed', 'inactive']) policy.activate(key);
    policy.remove('removed');
    expect(
      policy.reconcile([{ key: 'inactive', cost: 1, protected: false }], 0)
    ).toEqual(['inactive']);
    expect(policy.reconcile([], 0)).toEqual([]);
  });

  it('records reactivations after eviction without forgetting frequency history', () => {
    const { stats, policy } = fixture();
    policy.activate('page');
    const previous = stats.frequency('page');
    policy.remove('page');
    policy.activate('page');
    expect(stats.frequency('page')).toBe(previous + 1);
  });

  it('reconciles shrinking budgets by cost rather than a fixed page count', () => {
    const { candidates, activate, policy } = fixture();
    activate('a', 4, 12);
    activate('b', 3, 12);
    activate('c', 5, 12);
    const evictions = policy.reconcile([...candidates.values()], 8);
    for (const key of evictions) candidates.delete(key);
    expect(
      [...candidates.values()].reduce((total, page) => total + page.cost, 0)
    ).toBeLessThanOrEqual(8);
    expect(candidates.has('c')).toBe(true);
    expect(policy.reconcile([...candidates.values()], Infinity)).toEqual([]);
  });
});
