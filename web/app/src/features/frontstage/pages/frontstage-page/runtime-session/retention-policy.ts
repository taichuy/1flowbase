import type { RetentionStats } from './retention-stats';

type Segment = 'window' | 'probation' | 'protected';
type History = { segment: Segment; recency: number };
export type RetentionCandidate = {
  key: string;
  cost: number;
  protected: boolean;
};
type Resident = RetentionCandidate & History;

const WINDOW_SHARE = 0.2;
const PROTECTED_MAIN_SHARE = 0.8;

/**
 * W-TinyLFU adaptation for unequal-cost retained pages:
 * a recent admission window feeds a segmented-LRU main cache; an aged frequency
 * sketch rejects scans at the main admission boundary. Capacity is weighted by
 * caller-supplied cost, not page count or a claim of precise browser RAM usage.
 * Equal frequencies favor recent residents; a fitting latest window page gets
 * recency protection. Weight controls occupancy, not frequency ranking.
 * Multi-victim admission must defeat every victim before eviction is applied.
 * This owner holds only keys and numeric history, never React elements or DOM.
 */
export class RetentionPolicy {
  private readonly history = new Map<string, History>();
  private clock = 0;

  constructor(private readonly stats: RetentionStats) {}

  /** Caller records each successful activation once, including a cache miss. */
  activate(key: string): void {
    this.stats.record(key);
    const previous = this.history.get(key);
    this.history.set(key, {
      segment:
        previous?.segment === 'probation'
          ? 'protected'
          : (previous?.segment ?? 'window'),
      recency: ++this.clock
    });
  }

  remove(key: string): void {
    this.history.delete(key);
    // Eviction drops residency, not the aged frequency history.
  }

  reconcile(candidates: RetentionCandidate[], budget: number): string[] {
    const capacity = Number.isNaN(budget) ? 0 : Math.max(0, budget);
    const residents = new Map<string, Resident>();
    for (const candidate of candidates) {
      if (residents.has(candidate.key)) continue;
      let history = this.history.get(candidate.key);
      if (!history) {
        history = { segment: 'window', recency: ++this.clock };
        this.history.set(candidate.key, history);
      }
      residents.set(candidate.key, {
        ...candidate,
        cost: Number.isFinite(candidate.cost)
          ? Math.max(0, candidate.cost)
          : Number.MAX_VALUE,
        ...history
      });
    }
    const reserved = [...residents.values()]
      .filter((resident) => resident.protected)
      .reduce((total, resident) => total + resident.cost, 0);
    const available =
      capacity === Infinity ? Infinity : Math.max(0, capacity - reserved);
    const evictions: string[] = [];
    const evict = (resident: Resident) => {
      residents.delete(resident.key);
      this.remove(resident.key);
      evictions.push(resident.key);
    };
    const ordered = (segment: Segment) =>
      [...residents.values()]
        .filter(
          (resident) => !resident.protected && resident.segment === segment
        )
        .sort((left, right) => left.recency - right.recency);
    const totalCost = () =>
      [...residents.values()]
        .filter((resident) => !resident.protected)
        .reduce((total, resident) => total + resident.cost, 0);
    const move = (resident: Resident, segment: Segment) => {
      resident.segment = segment;
      this.history.get(resident.key)!.segment = segment;
    };
    const demoteMain = () => {
      const windowCost = ordered('window').reduce(
        (total, resident) => total + resident.cost,
        0
      );
      const protectedTarget =
        Math.max(0, available - windowCost) * PROTECTED_MAIN_SHARE;
      const protectedPages = ordered('protected');
      let protectedCost = protectedPages.reduce(
        (total, resident) => total + resident.cost,
        0
      );
      for (const resident of protectedPages) {
        if (protectedCost <= protectedTarget) break;
        move(resident, 'probation');
        protectedCost -= resident.cost;
      }
    };
    // A single page larger than available capacity cannot displace every fitting
    // resident and still survive; reject it before the normal admission contest.
    for (const resident of residents.values()) {
      if (!resident.protected && resident.cost > available) evict(resident);
    }
    demoteMain();
    while (true) {
      const window = ordered('window');
      const windowCost = window.reduce(
        (total, resident) => total + resident.cost,
        0
      );
      if (window.length <= 1 || windowCost <= available * WINDOW_SHARE) break;
      const incoming = window[0];
      move(incoming, 'probation');
      const excess = totalCost() - available;
      if (excess <= 0) continue;
      const victims: Resident[] = [];
      let recovered = 0;
      for (const victim of ordered('probation')) {
        if (victim.key === incoming.key) continue;
        victims.push(victim);
        recovered += victim.cost;
        if (recovered >= excess) break;
      }
      const incomingFrequency = this.stats.frequency(incoming.key);
      const losesAdmission = victims.some((victim) => {
        const victimFrequency = this.stats.frequency(victim.key);
        return (
          incomingFrequency < victimFrequency ||
          (incomingFrequency === victimFrequency &&
            incoming.recency <= victim.recency)
        );
      });
      if (recovered < excess || losesAdmission) {
        evict(incoming);
      } else {
        victims.forEach(evict);
      }
    }
    demoteMain();
    // Budget changes and protected reservations can require eviction without a
    // new admission. Prefer probation LRU, then protected LRU, then the window.
    for (const segment of ['probation', 'protected', 'window'] as const) {
      for (const resident of ordered(segment)) {
        if (totalCost() <= available) break;
        evict(resident);
      }
    }
    return evictions;
  }
}
