import { describe, expect, test } from 'vitest';
import {
  readStatisticsLogFilters,
  statisticsBucketFilters,
  statisticsLogsHref
} from '../lib/statistics-log-filters';

describe('statistics log links', () => {
  test('preserves exact half-open bounds and both model and user identities', () => {
    const filters = {
      started_from: '2026-09-22T02:30:00Z',
      started_to: '2026-09-22T03:00:00Z',
      requested_model_id: 'model/a+b',
      user_id: 'user-a'
    };
    const href = statisticsLogsHref('app-1', filters);
    expect(readStatisticsLogFilters(href.split('?')[1])).toEqual(filters);
  });
  test('clamps partial trend buckets to the exact report range', () => {
    const meta = {
      started_from: '2026-09-22T02:30:00Z',
      started_to: '2026-09-22T03:20:00Z'
    };
    expect(
      statisticsBucketFilters(meta, {
        bucket_start: '2026-09-22T02:00:00Z',
        bucket_end: '2026-09-22T03:00:00Z'
      })
    ).toEqual({
      started_from: meta.started_from,
      started_to: '2026-09-22T03:00:00Z'
    });
    expect(
      statisticsBucketFilters(meta, {
        bucket_start: '2026-09-22T03:00:00Z',
        bucket_end: '2026-09-22T04:00:00Z'
      })
    ).toEqual({
      started_from: '2026-09-22T03:00:00Z',
      started_to: meta.started_to
    });
  });
  test('preserves missing dimensions and ignores unrelated navigation fields', () => {
    expect(
      readStatisticsLogFilters(
        '?missing_model=true&missing_user=true&run_id=abc&view=trace'
      )
    ).toEqual({ missing_model: true, missing_user: true });
  });
});
