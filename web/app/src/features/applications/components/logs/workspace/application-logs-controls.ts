import { type PersistedRunArchiveImportJob } from './archive-import-persistence';

import {
  type ApplicationRunSortField,
  type ApplicationRunSortOrder
} from '../../../api/runtime';

export const RUN_ARCHIVE_IMPORT_CHUNK_SIZE = 1024 * 1024;

export const RUN_ARCHIVE_IMPORT_POLL_INTERVAL_MS = 1_000;

export const RUN_ARCHIVE_IMPORT_MAX_POLLS = 120;

export const DEFAULT_TIME_RANGE = '7';

export const ARCHIVE_IMPORT_TARGET_KEY =
  '1flowbase.application.all-agent-flow.run_archive_import_target';

export const EMPTY_APPLICATIONS: Array<{ id: string; name: string }> = [];

export const ALL_AGENT_FLOW_DEFAULT_COLUMN_KEYS = new Set([
  'title',
  'application_id',
  'requested_model_id',
  'reasoning_effort',
  'compatibility_mode',
  'status',
  'total_cost',
  'total_tokens',
  'input_cache_hit_rate',
  'started_at',
  'action'
]);

export type ApplicationLogTimeRange = '1' | '7' | '28' | '90' | '365' | 'all';

export type ApplicationLogsFloatingWindowKind =
  | 'conversation-log'
  | 'resume-timeline'
  | 'run-detail';

export type RunArchiveImportState = {
  phase: 'uploading' | 'processing';
  percent: number;
  fileName: string;
  jobId?: string;
  jobStatus?: string;
};

export function buildArchiveImportStateFromPersistedJob(
  job: PersistedRunArchiveImportJob | null
): RunArchiveImportState | null {
  if (!job) {
    return null;
  }

  return {
    phase: 'processing',
    percent: 90,
    fileName: job.fileName,
    jobId: job.jobId,
    jobStatus: 'queued'
  };
}

export const TIME_RANGE_OPTIONS: Array<{
  labelKey: string;
  value: ApplicationLogTimeRange;
}> = [
  { labelKey: 'auto.today', value: '1' },
  { labelKey: 'auto.past_seven_days', value: '7' },
  { labelKey: 'auto.past_four_weeks', value: '28' },
  { labelKey: 'auto.past_three_months', value: '90' },
  { labelKey: 'auto.past_twelve_months', value: '365' },
  { labelKey: 'auto.all_time', value: 'all' }
];

export const RUN_SORT_FIELD_OPTIONS: Array<{
  labelKey: string;
  value: ApplicationRunSortField;
}> = [
  { labelKey: 'auto.start_time', value: 'started_at' },
  { labelKey: 'auto.updated_at', value: 'updated_at' }
];

export const DEFAULT_SORT_BY: ApplicationRunSortField = 'started_at';

export const DEFAULT_SORT_ORDER: ApplicationRunSortOrder = 'desc';

export function getSortOrderToggleLabel(
  sortOrder: ApplicationRunSortOrder,
  t: (key: string) => string
) {
  return sortOrder === 'desc'
    ? t('auto.sort_descending_toggle_to_ascending')
    : t('auto.sort_ascending_toggle_to_descending');
}

export function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

type ApplicationLogsSearchState = {
  runId: string | null;
  view: 'trace' | null;
};

export function readApplicationLogsSearchState(): ApplicationLogsSearchState {
  if (typeof window === 'undefined') {
    return { runId: null, view: null };
  }

  const searchParams = new URLSearchParams(window.location.search);
  const runId = nonEmptyString(searchParams.get('run_id'));

  return {
    runId,
    view: runId && searchParams.get('view') === 'trace' ? 'trace' : null
  };
}

export function writeApplicationLogsSearchState(
  state: ApplicationLogsSearchState,
  mode: 'push' | 'replace' = 'push'
) {
  if (typeof window === 'undefined') {
    return;
  }

  const url = new URL(window.location.href);
  if (state.runId) {
    url.searchParams.set('run_id', state.runId);
  } else {
    url.searchParams.delete('run_id');
  }
  if (state.runId && state.view === 'trace') {
    url.searchParams.set('view', 'trace');
  } else {
    url.searchParams.delete('view');
  }

  window.history[mode === 'push' ? 'pushState' : 'replaceState'](
    {},
    '',
    `${url.pathname}${url.search}${url.hash}`
  );
}
