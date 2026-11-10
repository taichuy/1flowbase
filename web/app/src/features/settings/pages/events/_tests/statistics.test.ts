import type {
  EventRuntimeQueueSnapshot,
  EventSourceRuntimeSnapshot
} from '@1flowbase/api-client';
import { describe, expect, test } from 'vitest';
import {
  appendQueueSample,
  queueStatistics
} from '../runtime/statistics/queue-statistics';
import {
  appendObservationSample,
  formatObservationBytes,
  observationStatistics
} from '../runtime/statistics/observation-statistics';

function view(
  overrides: Partial<EventSourceRuntimeSnapshot> = {}
): EventSourceRuntimeSnapshot {
  return {
    instance_id: 'host',
    configured: true,
    scope: { kind: 'workspace', workspace_id: 'workspace' },
    observation_max_bytes: 4194304,
    observation_ttl_ms: 300000,
    sources: [],
    workflow_queues: [],
    activity: [],
    observation_bytes: 1024,
    source_metadata_bytes: 128,
    observation_loss: {
      accounting_available: true,
      window_started_at_unix_ms: 1000,
      window_duration_ms: 600000,
      expired: 78,
      evicted: 2,
      oversize_dropped: 3
    },
    ...overrides
  };
}
function queue(
  overrides: Partial<EventRuntimeQueueSnapshot> = {}
): EventRuntimeQueueSnapshot {
  return {
    subscription_id: 'one',
    workspace_id: 'workspace',
    contract_id: 'event',
    trigger_phase: 'published',
    consumer_ids: ['consumer-a', 'consumer-b'],
    closed: false,
    buffered: 3,
    in_flight: 2,
    accepted: 10,
    consumed: 7,
    admission_rejected: 0,
    ...overrides
  };
}

describe('report data contracts', () => {
  test('keeps the shared host limit single while summing disjoint visible scope charges', () => {
    const result = observationStatistics([
      view(),
      view({
        scope: { kind: 'system' },
        observation_bytes: 512,
        source_metadata_bytes: 64
      })
    ]);
    expect(result.observation_bytes).toBe(1536);
    expect(result.source_metadata_bytes).toBe(192);
    expect(result.observation_max_bytes).toBe(4194304);
  });
  test('preserves available loss counts without inventing totals for unavailable scopes', () => {
    const unavailable = view({
      scope: { kind: 'system' },
      observation_loss: {
        ...view().observation_loss,
        accounting_available: false,
        expired: 0,
        evicted: 0,
        oversize_dropped: 0
      }
    });
    const result = observationStatistics([view(), unavailable]);
    expect(result.accounting_scopes).toBe(1);
    expect(result.visible_scopes).toBe(2);
    expect(result.known_expired).toBe(78);
    expect(result.known_evicted).toBe(2);
    expect(result.known_oversize_dropped).toBe(3);
    expect(result.expired).toBeNull();
  });
  test('only exposes complete totals when every scope provides loss accounting', () => {
    const result = observationStatistics([
      view(),
      view({ scope: { kind: 'system' } })
    ]);
    expect(result.expired).toBe(156);
    expect(result.evicted).toBe(4);
    expect(result.oversize_dropped).toBe(6);
  });
  test('queue metrics use queue memberships and independent backend counters', () => {
    const result = queueStatistics([
      queue(),
      queue({
        subscription_id: 'two',
        closed: true,
        consumer_ids: ['consumer-a'],
        buffered: 5,
        in_flight: 1,
        accepted: 20,
        consumed: 15,
        admission_rejected: 2
      })
    ]);
    expect(result).toEqual({
      queue_count: 2,
      open: 1,
      closed: 1,
      buffered: 8,
      in_flight: 3,
      accepted: 30,
      consumed: 22,
      admission_rejected: 2,
      consumer_count: 3
    });
    expect(queueStatistics([]).queue_count).toBe(0);
  });
  test('formats raw byte charges as readable binary units', () => {
    expect(formatObservationBytes(0)).toBe('0 B');
    expect(formatObservationBytes(1024)).toBe('1 KiB');
    expect(formatObservationBytes(4194304)).toBe('4 MiB');
  });
});

describe('bounded visit-local chart samples', () => {
  test('a successful fetch timestamp contributes at most one point', () => {
    const points = [{ captured_at: 5000, buffered: 2 }];
    expect(appendQueueSample(points, { captured_at: 5000, buffered: 9 })).toBe(
      points
    );
    expect(
      appendQueueSample(points, { captured_at: 10000, buffered: 9 })
    ).toEqual([...points, { captured_at: 10000, buffered: 9 }]);
  });
  test('prunes samples outside the last thirty minutes', () => {
    const result = appendObservationSample(
      [
        { captured_at: 0, observation_bytes: 1000, source_metadata_bytes: 20 },
        {
          captured_at: 1800000,
          observation_bytes: 2000,
          source_metadata_bytes: 20
        }
      ],
      {
        captured_at: 1800001,
        observation_bytes: 3000,
        source_metadata_bytes: 20
      }
    );
    expect(result.map((point) => point.captured_at)).toEqual([
      1800000, 1800001
    ]);
  });
  test('caps point count even when manual refreshes are faster than polling', () => {
    const queuePoints = Array.from({ length: 400 }, (_, captured_at) => ({
      captured_at,
      buffered: 1
    }));
    expect(
      appendQueueSample(queuePoints, { captured_at: 400, buffered: 2 })
    ).toHaveLength(361);
    const recordPoints = queuePoints.map((point) => ({
      captured_at: point.captured_at,
      observation_bytes: 1,
      source_metadata_bytes: 0
    }));
    expect(
      appendObservationSample(recordPoints, {
        captured_at: 400,
        observation_bytes: 2,
        source_metadata_bytes: 0
      })
    ).toHaveLength(361);
  });
});
