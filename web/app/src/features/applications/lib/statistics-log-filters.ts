export const statisticsFilterKeys = [
  'started_from',
  'started_to',
  'requested_model_id',
  'user_id',
  'missing_model',
  'missing_user'
] as const;
export interface StatisticsLogFilters {
  started_from?: string;
  started_to?: string;
  requested_model_id?: string;
  user_id?: string;
  missing_model?: boolean;
  missing_user?: boolean;
}
export function readStatisticsLogFilters(search: string): StatisticsLogFilters {
  const params = new URLSearchParams(search);
  return Object.fromEntries(
    statisticsFilterKeys.flatMap((key) => {
      const value = params.get(key);
      return value
        ? [
            [
              key,
              key === 'missing_model' || key === 'missing_user'
                ? value === 'true'
                : value
            ]
          ]
        : [];
    })
  );
}
export function statisticsLogsHref(
  applicationId: string,
  filters: StatisticsLogFilters
) {
  const params = new URLSearchParams();
  for (const key of statisticsFilterKeys) {
    const value = filters[key];
    if (value !== undefined) params.set(key, String(value));
  }
  return `/applications/${encodeURIComponent(applicationId)}/logs?${params}`;
}

export function statisticsBucketFilters(
  meta: { started_from: string | null; started_to: string | null },
  point: { bucket_start: string; bucket_end: string }
): StatisticsLogFilters {
  return {
    started_from:
      meta.started_from &&
      Date.parse(meta.started_from) > Date.parse(point.bucket_start)
        ? meta.started_from
        : point.bucket_start,
    started_to:
      meta.started_to &&
      Date.parse(meta.started_to) < Date.parse(point.bucket_end)
        ? meta.started_to
        : point.bucket_end
  };
}
