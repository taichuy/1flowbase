import type { EventSourceRuntimeSnapshot } from '@1flowbase/api-client';

export function observationStatistics(views: EventSourceRuntimeSnapshot[]) {
  const accounting = views.filter(
    (view) => view.observation_loss.accounting_available
  );
  const complete = accounting.length === views.length && views.length > 0;
  return {
    observation_bytes: views.reduce(
      (sum, view) => sum + view.observation_bytes,
      0
    ),
    source_metadata_bytes: views.reduce(
      (sum, view) => sum + view.source_metadata_bytes,
      0
    ),
    retained_records: views.reduce(
      (sum, view) => sum + view.activity.length,
      0
    ),
    // Every scope shares the host budget; the denominator must not be summed.
    observation_max_bytes: views[0]?.observation_max_bytes ?? 0,
    observation_ttl_ms: views[0]?.observation_ttl_ms ?? 0,
    window_duration_ms: views[0]?.observation_loss.window_duration_ms ?? 0,
    accounting_scopes: accounting.length,
    visible_scopes: views.length,
    expired: complete
      ? accounting.reduce((sum, view) => sum + view.observation_loss.expired, 0)
      : null,
    evicted: complete
      ? accounting.reduce((sum, view) => sum + view.observation_loss.evicted, 0)
      : null,
    oversize_dropped: complete
      ? accounting.reduce(
          (sum, view) => sum + view.observation_loss.oversize_dropped,
          0
        )
      : null,
    known_expired: accounting.reduce(
      (sum, view) => sum + view.observation_loss.expired,
      0
    ),
    known_evicted: accounting.reduce(
      (sum, view) => sum + view.observation_loss.evicted,
      0
    ),
    known_oversize_dropped: accounting.reduce(
      (sum, view) => sum + view.observation_loss.oversize_dropped,
      0
    )
  };
}

export interface ObservationSample {
  captured_at: number;
  observation_bytes: number;
  source_metadata_bytes: number;
}

export function appendObservationSample(
  samples: ObservationSample[],
  sample: ObservationSample
) {
  if (samples.at(-1)?.captured_at === sample.captured_at) return samples;
  return [
    ...samples.filter(
      (point) => point.captured_at >= sample.captured_at - 30 * 60 * 1000
    ),
    sample
  ].slice(-361);
}

export function formatObservationBytes(bytes: number) {
  const units = ['B', 'KiB', 'MiB', 'GiB'];
  const index =
    bytes > 0
      ? Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
      : 0;
  const value = bytes / 1024 ** index;
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: index ? 1 : 0 }).format(value)} ${units[index]}`;
}
