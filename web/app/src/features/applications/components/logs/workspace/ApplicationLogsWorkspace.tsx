import {
  getViewportSize,
  getRunDetailInitialRect,
  getConversationLogInitialRect,
  getResumeTimelineInitialRect,
  resolveCollision
} from './floating-window-layout';
import {
  readStatisticsLogFilters,
  statisticsFilterKeys
} from '../../../lib/statistics-log-filters';
import type { ConsoleApplicationType } from '@1flowbase/api-client';
import { fetchRunPayload } from '../../../api/trajectory';
import DownloadOutlined from '@ant-design/icons/es/icons/DownloadOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import SortAscendingOutlined from '@ant-design/icons/es/icons/SortAscendingOutlined';
import SortDescendingOutlined from '@ant-design/icons/es/icons/SortDescendingOutlined';
import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Empty,
  Input,
  Progress,
  Modal,
  Select,
  Spin,
  Tooltip
} from 'antd';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type Key,
  type ReactNode
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { AutosizeSelect } from '../../../../../shared/ui/autosize-select/AutosizeSelect';
import type { AgentFlowDebugMessage } from '../../../../agent-flow/api/runtime';
import { ConversationLogPanel } from '../../../../agent-flow/components/debug-console/ConversationLogPanel';
import {
  applicationRunsQueryKey,
  completeApplicationRunArchiveUploadSession,
  createApplicationRunArchiveUploadSession,
  fetchApplicationRunArchiveImportJob,
  fetchApplicationRuns,
  fetchApplicationRunOverview,
  exportApplicationRunTraceDump,
  exportSelectedApplicationRunsTraceDumpZip,
  uploadApplicationRunArchiveChunk,
  type FetchApplicationRunsInput,
  type ApplicationRunArchiveImportJob,
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts,
  type ApplicationRunSortField,
  type ApplicationRunSortOrder,
  type ApplicationRunSummary
} from '../../../api/runtime';
import { ApplicationRunDetailPanel } from '../ApplicationRunDetailPanel';
import {
  buildApplicationRunTraceMessage,
  createApplicationLogTraceLoader
} from '../application-log-trace-loader';
import { ApplicationLogsFloatingWindow } from '../ApplicationLogsFloatingWindow';
import { ApplicationRunResumeTimelinePanel } from '../ApplicationRunResumeTimelinePanel';
import {
  clampRect,
  applyStoredWidth,
  DEFAULT_MIN_WIDTH,
  DEFAULT_MIN_HEIGHT,
  type FloatingWindowRect
} from '../floating-window-geometry';
import { getApplicationRunsTableColumns } from '../application-runs-table-columns';
import {
  ApplicationRunsTable,
  ApplicationRunsTableColumnSettings
} from '../ApplicationRunsTable';
import { useApplicationRunsTableConfiguration } from '../useApplicationRunsTableConfiguration';
import {
  buildRunTraceDumpFilename,
  buildSelectedRunTraceDumpFilename,
  saveApplicationRunExport
} from '../../../lib/run-export-download';
import { sha256ArrayBuffer } from '../../../lib/run-archive-hash';
import { useAuthStore } from '../../../../../state/auth-store';
import '../../../pages/application-logs-page.css';

const RUN_ARCHIVE_IMPORT_CHUNK_SIZE = 1024 * 1024;
const RUN_ARCHIVE_IMPORT_POLL_INTERVAL_MS = 1_000;
const RUN_ARCHIVE_IMPORT_MAX_POLLS = 120;
const DEFAULT_TIME_RANGE = '7';
const PAGE_SIZE = 20;
const ARCHIVE_IMPORT_TARGET_KEY =
  '1flowbase.application.all-agent-flow.run_archive_import_target';
const EMPTY_APPLICATIONS: Array<{ id: string; name: string }> = [];
const ALL_AGENT_FLOW_DEFAULT_COLUMN_KEYS = new Set([
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

type ApplicationLogTimeRange = '1' | '7' | '28' | '90' | '365' | 'all';
type ApplicationLogsFloatingWindowKind =
  | 'conversation-log'
  | 'resume-timeline'
  | 'run-detail';

type RunArchiveImportState = {
  phase: 'uploading' | 'processing';
  percent: number;
  fileName: string;
  jobId?: string;
  jobStatus?: string;
};

type PersistedRunArchiveImportJob = {
  jobId: string;
  fileName: string;
};

function buildArchiveImportStateFromPersistedJob(
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

const TIME_RANGE_OPTIONS: Array<{
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
const RUN_SORT_FIELD_OPTIONS: Array<{
  labelKey: string;
  value: ApplicationRunSortField;
}> = [
  { labelKey: 'auto.start_time', value: 'started_at' },
  { labelKey: 'auto.updated_at', value: 'updated_at' }
];
const DEFAULT_SORT_BY: ApplicationRunSortField = 'started_at';
const DEFAULT_SORT_ORDER: ApplicationRunSortOrder = 'desc';

function getSortOrderToggleLabel(
  sortOrder: ApplicationRunSortOrder,
  t: (key: string) => string
) {
  return sortOrder === 'desc'
    ? t('auto.sort_descending_toggle_to_ascending')
    : t('auto.sort_ascending_toggle_to_descending');
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

type ApplicationLogsSearchState = {
  runId: string | null;
  view: 'trace' | null;
};

function readApplicationLogsSearchState(): ApplicationLogsSearchState {
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

function writeApplicationLogsSearchState(
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

function archiveImportStorageKey(applicationId: string) {
  return `1flowbase.application.${applicationId}.run_archive_import_job`;
}

function readPersistedArchiveImportJob(
  applicationId: string
): PersistedRunArchiveImportJob | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const storedText = window.localStorage.getItem(
    archiveImportStorageKey(applicationId)
  );
  if (!storedText) {
    return null;
  }

  try {
    const parsedValue: unknown = JSON.parse(storedText);
    if (
      typeof parsedValue === 'object' &&
      parsedValue !== null &&
      typeof (parsedValue as PersistedRunArchiveImportJob).jobId === 'string' &&
      typeof (parsedValue as PersistedRunArchiveImportJob).fileName === 'string'
    ) {
      return parsedValue as PersistedRunArchiveImportJob;
    }
  } catch {
    window.localStorage.removeItem(archiveImportStorageKey(applicationId));
  }

  return null;
}

function writePersistedArchiveImportJob(
  applicationId: string,
  job: PersistedRunArchiveImportJob
) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(
    archiveImportStorageKey(applicationId),
    JSON.stringify(job)
  );
}

function clearPersistedArchiveImportJob(applicationId: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.removeItem(archiveImportStorageKey(applicationId));
}

export function ApplicationLogsWorkspace({
  applicationId,
  applicationIds,
  applications = EMPTY_APPLICATIONS,
  overlayContainer,
  applicationType = 'agent_flow'
}: {
  applicationId: string;
  applicationIds?: string[];
  applications?: Array<{ id: string; name: string }>;
  overlayContainer?: HTMLElement;
  applicationType?: ConsoleApplicationType;
}) {
  const runsScope = applicationIds ?? applicationId;
  const [selectedApplicationId, setSelectedApplicationId] = useState(
    () =>
      applicationId ||
      new URLSearchParams(window.location.search).get('application_id') ||
      ''
  );
  const [archiveApplicationId, setArchiveApplicationId] = useState(() => {
    const previousTarget = window.localStorage.getItem(
      ARCHIVE_IMPORT_TARGET_KEY
    );
    return (
      applicationId ||
      (applications.some((app) => app.id === previousTarget)
        ? previousTarget!
        : '')
    );
  });
  const archiveTargetApplicationId = applicationId || archiveApplicationId;
  const [importTargetOpen, setImportTargetOpen] = useState(false);
  const isWorkflow = applicationType === 'workflow';
  const [executionTab, setExecutionTab] = useState<'detail' | 'trace'>('trace');
  const { t } = useTranslation('applications');
  const initialSearchState = useMemo(readApplicationLogsSearchState, []);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(
    initialSearchState.runId
  );
  const [openConversationLogMessage, setOpenConversationLogMessage] =
    useState<AgentFlowDebugMessage | null>(() =>
      initialSearchState.runId && initialSearchState.view === 'trace'
        ? buildApplicationRunTraceMessage(initialSearchState.runId)
        : null
    );
  const [traceViewRequested, setTraceViewRequested] = useState(
    initialSearchState.view === 'trace'
  );
  const [openResumeTimelineRunId, setOpenResumeTimelineRunId] = useState<
    string | null
  >(null);
  const [runDetailRect, setRunDetailRect] = useState<FloatingWindowRect | null>(
    null
  );
  const [conversationLogRect, setConversationLogRect] =
    useState<FloatingWindowRect | null>(null);
  const [resumeTimelineRect, setResumeTimelineRect] =
    useState<FloatingWindowRect | null>(null);
  const [timeRange, setTimeRange] =
    useState<ApplicationLogTimeRange>(DEFAULT_TIME_RANGE);

  useEffect(() => {
    function handleViewportResize() {
      if (runDetailRect) {
        setRunDetailRect(
          clampRect(runDetailRect, DEFAULT_MIN_WIDTH, DEFAULT_MIN_HEIGHT)
        );
      }
      if (conversationLogRect) {
        setConversationLogRect(
          clampRect(conversationLogRect, DEFAULT_MIN_WIDTH, DEFAULT_MIN_HEIGHT)
        );
      }
      if (resumeTimelineRect) {
        setResumeTimelineRect(
          clampRect(resumeTimelineRect, DEFAULT_MIN_WIDTH, DEFAULT_MIN_HEIGHT)
        );
      }
    }

    window.addEventListener('resize', handleViewportResize);
    return () => window.removeEventListener('resize', handleViewportResize);
  }, [runDetailRect, conversationLogRect, resumeTimelineRect]);
  const [statisticsFilters, setStatisticsFilters] = useState(() =>
    readStatisticsLogFilters(window.location.search)
  );
  const clearStatisticsFilters = () => {
    const url = new URL(window.location.href);
    statisticsFilterKeys.forEach((key) => url.searchParams.delete(key));
    window.history.replaceState(window.history.state, '', url);
    setStatisticsFilters({});
    setPage(1);
  };
  const [keywordSearch, setKeywordSearch] = useState('');
  const [page, setPage] = useState(1);
  const [sortBy, setSortBy] =
    useState<ApplicationRunSortField>(DEFAULT_SORT_BY);
  const [sortOrder, setSortOrder] =
    useState<ApplicationRunSortOrder>(DEFAULT_SORT_ORDER);
  const [refreshingRuns, setRefreshingRuns] = useState(false);
  const [exportingSelectedRuns, setExportingSelectedRuns] = useState(false);
  const [archiveImportState, setArchiveImportState] =
    useState<RunArchiveImportState | null>(() =>
      buildArchiveImportStateFromPersistedJob(
        readPersistedArchiveImportJob(archiveTargetApplicationId)
      )
    );
  const [exportingRunId, setExportingRunId] = useState<string | null>(null);
  const [selectedRunIds, setSelectedRunIds] = useState<string[]>([]);
  const [activeFloatingWindow, setActiveFloatingWindow] =
    useState<ApplicationLogsFloatingWindowKind>(
      !isWorkflow && initialSearchState.view === 'trace'
        ? 'conversation-log'
        : 'run-detail'
    );
  const archiveImportInputRef = useRef<HTMLInputElement | null>(null);
  const restoringArchiveImportRef = useRef(false);
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const timeRangeOptions = useMemo(
    () =>
      TIME_RANGE_OPTIONS.map((option) => ({
        label: t(option.labelKey),
        value: option.value
      })),
    [t]
  );
  const runSortFieldOptions = useMemo(
    () =>
      RUN_SORT_FIELD_OPTIONS.map((option) => ({
        label: t(option.labelKey),
        value: option.value
      })),
    [t]
  );
  const runSortFieldMeasureLabels = useMemo(
    () =>
      runSortFieldOptions.map((option) =>
        t('auto.sort_by_value', { value1: option.label })
      ),
    [runSortFieldOptions, t]
  );
  const runsTableColumns = useMemo(() => {
    const columns = getApplicationRunsTableColumns(t, Boolean(applicationIds));
    if (applicationIds) {
      columns.splice(1, 0, {
        key: 'application_id',
        dataIndex: 'application_id',
        title: t('logs.application'),
        width: 180,
        ellipsis: true,
        render: (_value, run) =>
          applications.find((app) => app.id === run.application_id)?.name ??
          run.application_id
      });
    }
    return applicationIds
      ? columns.map((column) => ({
          ...column,
          defaultVisibility: ALL_AGENT_FLOW_DEFAULT_COLUMN_KEYS.has(column.key)
            ? ('visible' as const)
            : ('hidden' as const)
        }))
      : columns;
  }, [applicationIds, applications, t]);
  const runsTableConfiguration = useApplicationRunsTableConfiguration(
    runsTableColumns,
    applicationIds ? 'applications.logs.all-agent-flow-runs' : undefined
  );
  const titleIncludes = keywordSearch.trim();
  const runsInput: FetchApplicationRunsInput = useMemo(
    () => ({
      page,
      pageSize: PAGE_SIZE,
      ...statisticsFilters,
      timeRangeDays:
        statisticsFilters.started_from ||
        statisticsFilters.started_to ||
        timeRange === 'all'
          ? null
          : Number(timeRange),
      sortBy,
      sortOrder,
      titleIncludes: titleIncludes || undefined
    }),
    [page, sortBy, sortOrder, timeRange, titleIncludes, statisticsFilters]
  );
  const runsQuery = useQuery({
    queryKey: applicationRunsQueryKey(runsScope, runsInput),
    queryFn: () => fetchApplicationRuns(runsScope, runsInput)
  });
  const runsPage = runsQuery.data;
  const runs = useMemo(() => runsPage?.items ?? [], [runsPage?.items]);
  const total = runsPage?.total ?? 0;
  const visibleRunIds = useMemo(
    () => new Set(runs.map((run) => run.id)),
    [runs]
  );
  const selectedVisibleRunIds = useMemo(
    () => selectedRunIds.filter((runId) => visibleRunIds.has(runId)),
    [selectedRunIds, visibleRunIds]
  );

  useEffect(() => {
    setPage(1);
  }, [applicationId]);

  useEffect(() => {
    function applyLocationSearch() {
      setStatisticsFilters(readStatisticsLogFilters(window.location.search));
      setPage(1);
      const searchState = readApplicationLogsSearchState();
      setSelectedApplicationId(
        applicationId ||
          new URLSearchParams(window.location.search).get('application_id') ||
          ''
      );
      setSelectedRunId(searchState.runId);
      setExecutionTab('trace');
      setOpenConversationLogMessage(
        searchState.runId && searchState.view === 'trace'
          ? buildApplicationRunTraceMessage(searchState.runId)
          : null
      );
      setTraceViewRequested(searchState.view === 'trace');
      setOpenResumeTimelineRunId(null);
      setActiveFloatingWindow(
        !isWorkflow && searchState.view === 'trace'
          ? 'conversation-log'
          : 'run-detail'
      );
      setRunDetailRect(null);
      setConversationLogRect(null);
      setResumeTimelineRect(null);
    }

    window.addEventListener('popstate', applyLocationSearch);
    return () => window.removeEventListener('popstate', applyLocationSearch);
  }, [applicationId, isWorkflow]);

  function selectRun(run: ApplicationRunSummary | null) {
    const nextRunId = run ? run.id : null;
    writeApplicationLogsSearchState({ runId: nextRunId, view: null });
    setSelectedApplicationId(run?.application_id ?? applicationId);
    if (applicationIds) {
      const url = new URL(window.location.href);
      if (run) url.searchParams.set('application_id', run.application_id);
      else url.searchParams.delete('application_id');
      window.history.replaceState(window.history.state, '', url);
    }
    setSelectedRunId(nextRunId);
    setExecutionTab('trace');
    setTraceViewRequested(false);
    setOpenConversationLogMessage(null);
    setOpenResumeTimelineRunId(null);
    setActiveFloatingWindow('run-detail');

    if (nextRunId) {
      const initial = clampRect(
        applyStoredWidth(
          getRunDetailInitialRect(),
          'application-logs-floating-run-detail'
        ),
        DEFAULT_MIN_WIDTH,
        DEFAULT_MIN_HEIGHT
      );
      setRunDetailRect(initial);
    } else {
      setRunDetailRect(null);
    }
    setConversationLogRect(null);
    setResumeTimelineRect(null);
  }

  const handleRectChange = (
    type: ApplicationLogsFloatingWindowKind,
    newRect: FloatingWindowRect
  ) => {
    if (type === 'run-detail') {
      setRunDetailRect(newRect);
    } else if (type === 'conversation-log') {
      setConversationLogRect(newRect);
    } else {
      setResumeTimelineRect(newRect);
    }
  };

  function toggleSortOrder() {
    setSortOrder((current) => (current === 'desc' ? 'asc' : 'desc'));
    setPage(1);
    setSelectedRunIds([]);
  }

  function changeTimeRange(nextTimeRange: ApplicationLogTimeRange) {
    setTimeRange(nextTimeRange);
    setPage(1);
    setSelectedRunIds([]);
  }

  function changeSortBy(nextSortBy: ApplicationRunSortField) {
    setSortBy(nextSortBy);
    setPage(1);
    setSelectedRunIds([]);
  }

  function changeKeywordSearch(event: ChangeEvent<HTMLInputElement>) {
    setKeywordSearch(event.target.value);
    setPage(1);
    setSelectedRunIds([]);
  }

  function changePage(nextPage: number) {
    setPage(nextPage);
    setSelectedRunIds([]);
  }

  async function refreshRunsFromDurable() {
    setSelectedRunIds([]);
    setRefreshingRuns(true);
    try {
      const refreshedRuns = await fetchApplicationRuns(runsScope, {
        ...runsInput,
        cacheMode: 'refresh'
      });
      queryClient.setQueryData(
        applicationRunsQueryKey(runsScope, runsInput),
        refreshedRuns
      );
    } catch {
      message.error(t('auto.refresh_failed'));
    } finally {
      setRefreshingRuns(false);
    }
  }

  async function exportSelectedRuns() {
    const runIds = selectedRunIds.filter((runId) => visibleRunIds.has(runId));

    if (runIds.length === 0) {
      return;
    }

    if (!csrfToken) {
      message.error(t('auto.export_logs_csrf_missing'));
      return;
    }

    setExportingSelectedRuns(true);
    try {
      const groups = new Map<string, string[]>();
      for (const runId of runIds) {
        const owner =
          applicationId || runs.find((run) => run.id === runId)!.application_id;
        groups.set(owner, [...(groups.get(owner) ?? []), runId]);
      }
      for (const [owner, ownerRunIds] of groups) {
        const download = await exportSelectedApplicationRunsTraceDumpZip(
          owner,
          ownerRunIds,
          csrfToken
        );
        saveApplicationRunExport(
          download,
          groups.size === 1
            ? buildSelectedRunTraceDumpFilename()
            : `${owner}-${buildSelectedRunTraceDumpFilename()}`
        );
      }
    } catch {
      message.error(t('auto.export_logs_failed'));
    } finally {
      setExportingSelectedRuns(false);
    }
  }

  const waitForArchiveImportJob = useCallback(
    async (jobId: string, fileName: string) => {
      let lastJob: ApplicationRunArchiveImportJob | null = null;

      async function poll(
        index: number
      ): Promise<ApplicationRunArchiveImportJob | null> {
        if (index >= RUN_ARCHIVE_IMPORT_MAX_POLLS) {
          return lastJob;
        }

        const job = await fetchApplicationRunArchiveImportJob(
          archiveTargetApplicationId,
          jobId
        );
        lastJob = job;
        setArchiveImportState({
          phase: 'processing',
          percent: job.status === 'succeeded' ? 100 : 90,
          fileName,
          jobId,
          jobStatus: job.status
        });
        if (job.status === 'succeeded' || job.status === 'failed') {
          return job;
        }
        await new Promise((resolve) =>
          window.setTimeout(resolve, RUN_ARCHIVE_IMPORT_POLL_INTERVAL_MS)
        );

        return poll(index + 1);
      }

      return poll(0);
    },
    [archiveTargetApplicationId]
  );

  const finishArchiveImportJob = useCallback(
    async (job: ApplicationRunArchiveImportJob | null) => {
      if (job && (job.status === 'succeeded' || job.status === 'failed')) {
        clearPersistedArchiveImportJob(archiveTargetApplicationId);
      }

      if (!job || job.status !== 'succeeded') {
        message.error(t('auto.import_run_archive_failed'));
        return;
      }

      await queryClient.invalidateQueries({
        queryKey: applicationRunsQueryKey(runsScope, runsInput)
      });
      const targetRunId =
        job.source_to_target_run_ids[0]?.target_run_id ?? null;
      if (targetRunId) {
        setSelectedApplicationId(archiveTargetApplicationId);
        setSelectedRunId(targetRunId);
        setActiveFloatingWindow('run-detail');
        setRunDetailRect(
          clampRect(
            applyStoredWidth(
              getRunDetailInitialRect(),
              'application-logs-floating-run-detail'
            ),
            DEFAULT_MIN_WIDTH,
            DEFAULT_MIN_HEIGHT
          )
        );
      }
      message.success(
        t('auto.import_run_archive_succeeded', {
          value1: job.imported_run_count
        })
      );
    },
    [archiveTargetApplicationId, message, queryClient, runsInput, runsScope, t]
  );

  useEffect(() => {
    const persistedJob = readPersistedArchiveImportJob(
      archiveTargetApplicationId
    );
    if (!persistedJob || restoringArchiveImportRef.current) {
      return;
    }

    restoringArchiveImportRef.current = true;
    void (async () => {
      try {
        const job = await waitForArchiveImportJob(
          persistedJob.jobId,
          persistedJob.fileName
        );
        await finishArchiveImportJob(job);
      } finally {
        restoringArchiveImportRef.current = false;
        setArchiveImportState(null);
      }
    })();
  }, [
    archiveTargetApplicationId,
    finishArchiveImportJob,
    waitForArchiveImportJob
  ]);

  async function importRunArchiveFile(file: File) {
    if (!csrfToken) {
      message.error(t('auto.import_run_archive_csrf_missing'));
      return;
    }
    const archiveCsrfToken = csrfToken;

    setArchiveImportState({
      phase: 'uploading',
      percent: 0,
      fileName: file.name
    });
    try {
      const fileBuffer = await file.arrayBuffer();
      const archiveSha256 = await sha256ArrayBuffer(fileBuffer);
      const session = await createApplicationRunArchiveUploadSession(
        archiveTargetApplicationId,
        {
          filename: file.name,
          total_size_bytes: file.size,
          expected_sha256: archiveSha256,
          chunk_size_bytes: RUN_ARCHIVE_IMPORT_CHUNK_SIZE
        },
        archiveCsrfToken
      );

      const chunkCount = Math.max(
        1,
        Math.ceil(file.size / RUN_ARCHIVE_IMPORT_CHUNK_SIZE)
      );
      async function uploadChunk(chunkIndex: number): Promise<void> {
        if (chunkIndex >= chunkCount) {
          return;
        }

        const start = chunkIndex * RUN_ARCHIVE_IMPORT_CHUNK_SIZE;
        const end = Math.min(file.size, start + RUN_ARCHIVE_IMPORT_CHUNK_SIZE);
        const chunk = file.slice(start, end);
        const chunkSha256 = await sha256ArrayBuffer(await chunk.arrayBuffer());
        await uploadApplicationRunArchiveChunk(
          archiveTargetApplicationId,
          session.session_id,
          chunkIndex,
          chunk,
          chunkSha256,
          archiveCsrfToken
        );
        setArchiveImportState({
          phase: 'uploading',
          percent: Math.round(((chunkIndex + 1) / chunkCount) * 80),
          fileName: file.name
        });

        return uploadChunk(chunkIndex + 1);
      }

      await uploadChunk(0);

      const queuedJob = await completeApplicationRunArchiveUploadSession(
        archiveTargetApplicationId,
        session.session_id,
        archiveCsrfToken
      );
      setArchiveImportState({
        phase: 'processing',
        percent: 90,
        fileName: file.name,
        jobId: queuedJob.job_id,
        jobStatus: queuedJob.status
      });
      writePersistedArchiveImportJob(archiveTargetApplicationId, {
        jobId: queuedJob.job_id,
        fileName: file.name
      });
      const job = await waitForArchiveImportJob(queuedJob.job_id, file.name);
      await finishArchiveImportJob(job);
    } catch {
      message.error(t('auto.import_run_archive_failed'));
    } finally {
      setArchiveImportState(null);
    }
  }

  function handleArchiveImportInputChange(
    event: ChangeEvent<HTMLInputElement>
  ) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (file) {
      void importRunArchiveFile(file);
    }
  }

  async function exportRunTraceDump(runId: string) {
    setExportingRunId(runId);
    try {
      const download = await exportApplicationRunTraceDump(
        applicationId || selectedApplicationId,
        runId
      );
      saveApplicationRunExport(download, buildRunTraceDumpFilename(runId));
    } catch {
      message.error(t('auto.export_logs_failed'));
    } finally {
      setExportingRunId(null);
    }
  }

  function changeLogTab(tab: 'detail' | 'trace') {
    setTraceViewRequested(tab === 'trace');
    writeApplicationLogsSearchState({
      runId: selectedRunId,
      view: tab === 'trace' ? 'trace' : null
    });
  }

  function openConversationLog(message: AgentFlowDebugMessage) {
    setTraceViewRequested(false);
    setOpenConversationLogMessage(message);
    setActiveFloatingWindow('conversation-log');

    const initial = clampRect(
      applyStoredWidth(
        getConversationLogInitialRect(),
        'application-logs-floating-conversation-log'
      ),
      DEFAULT_MIN_WIDTH,
      DEFAULT_MIN_HEIGHT
    );
    const anchorRect = resumeTimelineRect ?? runDetailRect;
    if (anchorRect) {
      const viewport = getViewportSize();
      const resolved = resolveCollision(anchorRect, initial, viewport.width);

      if (resumeTimelineRect) {
        setResumeTimelineRect(resolved.rectA);
      } else {
        setRunDetailRect(resolved.rectA);
      }
      setConversationLogRect(resolved.rectB);
    } else {
      setConversationLogRect(initial);
    }
  }

  function openResumeTimeline(message?: AgentFlowDebugMessage) {
    const targetRunId =
      nonEmptyString(message?.detailRunId) ??
      (message?.canOpenDetail === false
        ? null
        : nonEmptyString(message?.runId)) ??
      selectedRunId;

    if (!targetRunId) {
      return;
    }

    setOpenResumeTimelineRunId(targetRunId);
    setActiveFloatingWindow('resume-timeline');

    const initial = clampRect(
      applyStoredWidth(
        getResumeTimelineInitialRect(),
        'application-logs-floating-resume-timeline'
      ),
      DEFAULT_MIN_WIDTH,
      DEFAULT_MIN_HEIGHT
    );
    const anchorRect = conversationLogRect ?? runDetailRect;
    if (anchorRect) {
      const viewport = getViewportSize();
      const resolved = resolveCollision(anchorRect, initial, viewport.width);

      if (conversationLogRect) {
        setConversationLogRect(resolved.rectA);
      } else {
        setRunDetailRect(resolved.rectA);
      }
      setResumeTimelineRect(resolved.rectB);
    } else {
      setResumeTimelineRect(initial);
    }
  }

  const runsRowSelection = useMemo(
    () => ({
      selectedRowKeys: selectedVisibleRunIds,
      onChange: (nextSelectedRowKeys: Key[]) => {
        const nextRunIds: string[] = [];
        for (const key of nextSelectedRowKeys) {
          const runId = String(key);
          if (visibleRunIds.has(runId)) {
            nextRunIds.push(runId);
          }
        }
        setSelectedRunIds(nextRunIds);
      },
      getCheckboxProps: (run: ApplicationRunSummary) => ({
        name: run.id,
        'aria-label': t('auto.select_run_for_export', {
          value1: run.title || run.id
        })
      })
    }),
    [selectedVisibleRunIds, t, visibleRunIds]
  );

  const archiveImportStatus = archiveImportState ? (
    <div className="application-logs-page__archive-import-status" role="status">
      <span className="application-logs-page__archive-import-status-text">
        {archiveImportState.phase === 'uploading'
          ? t('auto.import_run_archive_uploading', {
              value1: archiveImportState.fileName
            })
          : t('auto.import_run_archive_processing', {
              value1: archiveImportState.fileName,
              value2: archiveImportState.jobStatus ?? 'queued'
            })}
      </span>
      <Progress
        className="application-logs-page__archive-import-progress"
        percent={archiveImportState.percent}
        size="small"
      />
    </div>
  ) : null;

  const traceLoader = createApplicationLogTraceLoader(
    applicationId || selectedApplicationId
  );

  const logsHeader = (
    <div className="application-logs-page__header">
      {Object.keys(statisticsFilters).length > 0 && (
        <Alert
          type="info"
          message={t('statistics.active_filters')}
          description={
            <>
              {statisticsFilters.started_from} – {statisticsFilters.started_to}
              {statisticsFilters.requested_model_id && (
                <span> · {statisticsFilters.requested_model_id}</span>
              )}
              {statisticsFilters.user_id && (
                <span> · {t('statistics.user_filter')}</span>
              )}
              {statisticsFilters.missing_model && (
                <span> · {t('statistics.unknown_model')}</span>
              )}
              {statisticsFilters.missing_user && (
                <span> · {t('statistics.unknown_user')}</span>
              )}
            </>
          }
          action={
            <Button onClick={clearStatisticsFilters}>
              {t('statistics.clear_filters')}
            </Button>
          }
        />
      )}
      <div className="application-logs-page__filters" role="search">
        <AutosizeSelect<ApplicationLogTimeRange>
          aria-label={t('auto.time_range')}
          options={timeRangeOptions}
          disabled={Boolean(
            statisticsFilters.started_from || statisticsFilters.started_to
          )}
          value={timeRange}
          onChange={changeTimeRange}
        />
        <span
          className="application-logs-page__sort-control"
          data-testid="application-logs-sort-control"
        >
          <AutosizeSelect<ApplicationRunSortField>
            aria-label={t('auto.sort_field')}
            autosizeLabels={runSortFieldMeasureLabels}
            className="application-logs-page__sort-select"
            options={runSortFieldOptions}
            prefix={
              <span className="application-logs-page__sort-select-prefix">
                {t('auto.sort_by_prefix')}
              </span>
            }
            value={sortBy}
            onChange={changeSortBy}
          />
          <Button
            aria-label={getSortOrderToggleLabel(sortOrder, t)}
            className="application-logs-page__sort-direction-button"
            icon={
              sortOrder === 'desc' ? (
                <SortDescendingOutlined aria-hidden="true" />
              ) : (
                <SortAscendingOutlined aria-hidden="true" />
              )
            }
            onClick={toggleSortOrder}
          />
        </span>
        <Input
          allowClear
          aria-label={t('auto.keyword_search')}
          className="application-logs-page__filter-search"
          placeholder={t('auto.search_title')}
          prefix={<SearchOutlined />}
          value={keywordSearch}
          onChange={changeKeywordSearch}
        />
        <div className="application-logs-page__filter-actions">
          <Tooltip title={t('auto.export_selected_runs_trace_dump')}>
            <Button
              aria-label={t('auto.export_selected_runs_trace_dump')}
              disabled={selectedVisibleRunIds.length === 0}
              icon={<UploadOutlined aria-hidden="true" />}
              loading={exportingSelectedRuns}
              onClick={() => {
                void exportSelectedRuns();
              }}
            />
          </Tooltip>
          <input
            ref={archiveImportInputRef}
            accept="application/json,.json,application/zip,.zip"
            data-testid="application-logs-archive-import-input"
            aria-label={t('auto.import_run_archive')}
            onChange={handleArchiveImportInputChange}
            style={{ display: 'none' }}
            type="file"
          />
          <Tooltip title={t('auto.import_run_archive')}>
            <Button
              aria-label={t('auto.import_run_archive')}
              disabled={
                archiveImportState !== null ||
                (applicationIds !== undefined && applications.length === 0)
              }
              icon={<DownloadOutlined aria-hidden="true" />}
              onClick={() =>
                applicationId
                  ? archiveImportInputRef.current?.click()
                  : setImportTargetOpen(true)
              }
            />
          </Tooltip>
          <Tooltip title={t('auto.refresh_logs')}>
            <Button
              aria-label={t('auto.refresh_logs')}
              icon={<ReloadOutlined aria-hidden="true" />}
              loading={refreshingRuns}
              onClick={() => {
                void refreshRunsFromDurable();
              }}
            />
          </Tooltip>
          <ApplicationRunsTableColumnSettings
            columns={runsTableColumns}
            configuration={runsTableConfiguration}
          />
        </div>
      </div>
    </div>
  );
  const logsList = (
    <section
      className="application-logs-page__list"
      data-testid="application-logs-list"
    >
      {runsQuery.isPending ? (
        <div className="application-logs-page__state" role="status">
          <Spin aria-hidden="true" />
          <span>{t('auto.logs_loading')}</span>
        </div>
      ) : runsQuery.isError ? (
        <Alert
          action={
            <Button
              size="small"
              onClick={() => {
                void runsQuery.refetch();
              }}
            >
              {t('auto.refresh_logs')}
            </Button>
          }
          description={t('auto.logs_load_failed_description')}
          title={t('auto.logs_load_failed')}
          showIcon
          type="error"
        />
      ) : runs.length === 0 ? (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={null} />
      ) : (
        <ApplicationRunsTable
          loading={runsQuery.isFetching}
          page={page}
          pageSize={PAGE_SIZE}
          total={total}
          configuration={runsTableConfiguration}
          columns={runsTableColumns}
          runs={runs}
          rowSelection={runsRowSelection}
          selectedRunId={selectedRunId}
          onPageChange={changePage}
          onSelectRun={selectRun}
        />
      )}
    </section>
  );

  function renderOverlays(children: ReactNode) {
    return overlayContainer
      ? createPortal(children, overlayContainer)
      : children;
  }

  return (
    <div className="application-logs-page" data-testid="application-logs-page">
      <Modal
        title={t('auto.import_run_archive')}
        open={importTargetOpen}
        onCancel={() => setImportTargetOpen(false)}
        okButtonProps={{ disabled: !archiveApplicationId }}
        onOk={() => {
          setImportTargetOpen(false);
          archiveImportInputRef.current?.click();
        }}
      >
        <Select
          aria-label={t('logs.import_target_application')}
          placeholder={t('logs.import_target_application')}
          style={{ width: '100%' }}
          value={archiveApplicationId || undefined}
          options={applications.map((app) => ({
            value: app.id,
            label: app.name
          }))}
          onChange={(targetApplicationId) => {
            setArchiveApplicationId(targetApplicationId);
            window.localStorage.setItem(
              ARCHIVE_IMPORT_TARGET_KEY,
              targetApplicationId
            );
          }}
        />
      </Modal>
      <div className="application-logs-page__stack">
        {logsHeader}
        {archiveImportStatus}
        {logsList}
      </div>
      {renderOverlays(
        <>
          {!isWorkflow && openConversationLogMessage ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'conversation-log'}
              initialRect={getConversationLogInitialRect}
              rect={conversationLogRect ?? undefined}
              onRectChange={(rect) =>
                handleRectChange('conversation-log', rect)
              }
              testId="application-logs-floating-conversation-log"
              title={t('auto.conversation_logs')}
              onActivate={() => setActiveFloatingWindow('conversation-log')}
            >
              <div className="application-logs-page__conversation-log-panel">
                <ConversationLogPanel
                  activeTab={traceViewRequested ? 'trace' : 'detail'}
                  onTabChange={changeLogTab}
                  defaultTraceToolsExpanded
                  message={openConversationLogMessage}
                  onClose={() => {
                    writeApplicationLogsSearchState({
                      runId: selectedRunId,
                      view: null
                    });
                    setTraceViewRequested(false);
                    setOpenConversationLogMessage(null);
                    setConversationLogRect(null);
                    setActiveFloatingWindow('run-detail');
                  }}
                  onLoadArtifact={(artifactRef) =>
                    fetchRuntimeDebugArtifact(
                      applicationId || selectedApplicationId,
                      artifactRef
                    )
                  }
                  onLoadArtifacts={(artifactRefs) =>
                    fetchRuntimeDebugArtifacts(
                      applicationId || selectedApplicationId,
                      artifactRefs
                    )
                  }
                  traceLoader={traceLoader}
                  overviewLoader={{
                    loadPayload: (runId, section) =>
                      fetchRunPayload(
                        applicationId || selectedApplicationId,
                        runId,
                        section
                      ),
                    loadOverview: (runId) =>
                      fetchApplicationRunOverview(
                        applicationId || selectedApplicationId,
                        runId
                      )
                  }}
                  exportingRun={
                    exportingRunId ===
                    (openConversationLogMessage.detailRunId ??
                      openConversationLogMessage.runId)
                  }
                  onExportRun={(runId) => {
                    void exportRunTraceDump(runId);
                  }}
                />
              </div>
            </ApplicationLogsFloatingWindow>
          ) : null}
          {openResumeTimelineRunId ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'resume-timeline'}
              initialRect={getResumeTimelineInitialRect}
              rect={resumeTimelineRect ?? undefined}
              onRectChange={(rect) => handleRectChange('resume-timeline', rect)}
              testId="application-logs-floating-resume-timeline"
              title={t('auto.resume_timeline')}
              onActivate={() => setActiveFloatingWindow('resume-timeline')}
            >
              <div className="application-logs-page__resume-timeline-panel">
                <ApplicationRunResumeTimelinePanel
                  applicationId={applicationId || selectedApplicationId}
                  runId={openResumeTimelineRunId}
                  onClose={() => {
                    setOpenResumeTimelineRunId(null);
                    setResumeTimelineRect(null);
                  }}
                />
              </div>
            </ApplicationLogsFloatingWindow>
          ) : null}
          {selectedRunId ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'run-detail'}
              initialRect={getRunDetailInitialRect}
              rect={runDetailRect ?? undefined}
              onRectChange={(rect) => handleRectChange('run-detail', rect)}
              testId="application-logs-floating-run-detail"
              title={t('auto.run_details')}
              onActivate={() => setActiveFloatingWindow('run-detail')}
            >
              {isWorkflow ? (
                <div className="application-logs-page__conversation-log-panel">
                  <ConversationLogPanel
                    key={selectedRunId}
                    title={t('auto.run_details')}
                    closeLabel={t('auto.close_run_details')}
                    traceLabel={t('auto.node_execution')}
                    activeTab={executionTab}
                    onTabChange={(tab) => {
                      setExecutionTab(tab);
                      changeLogTab(tab);
                    }}
                    defaultTraceToolsExpanded
                    message={buildApplicationRunTraceMessage(selectedRunId)}
                    onClose={() => selectRun(null)}
                    traceLoader={traceLoader}
                    overviewLoader={{
                      loadPayload: (runId, section) =>
                        fetchRunPayload(
                          applicationId || selectedApplicationId,
                          runId,
                          section
                        ),
                      loadOverview: (runId) =>
                        fetchApplicationRunOverview(
                          applicationId || selectedApplicationId,
                          runId
                        )
                    }}
                    onLoadArtifact={(artifactRef) =>
                      fetchRuntimeDebugArtifact(
                        applicationId || selectedApplicationId,
                        artifactRef
                      )
                    }
                    onLoadArtifacts={(artifactRefs) =>
                      fetchRuntimeDebugArtifacts(
                        applicationId || selectedApplicationId,
                        artifactRefs
                      )
                    }
                    exportingRun={exportingRunId === selectedRunId}
                    onExportRun={(runId) => {
                      void exportRunTraceDump(runId);
                    }}
                  />
                </div>
              ) : (
                <ApplicationRunDetailPanel
                  traceLoader={traceLoader}
                  applicationId={applicationId || selectedApplicationId}
                  requested_model_id={
                    runs.find((run) => run.id === selectedRunId)
                      ?.requested_model_id
                  }
                  reasoning_effort={
                    runs.find((run) => run.id === selectedRunId)
                      ?.reasoning_effort
                  }
                  logConversationId={
                    runs.find((run) => run.id === selectedRunId)
                      ?.log_conversation_id
                  }
                  onClose={() => selectRun(null)}
                  onOpenMessageLog={openConversationLog}
                  onOpenRunTrace={() => {
                    if (!selectedRunId) return;
                    openConversationLog(
                      buildApplicationRunTraceMessage(selectedRunId)
                    );
                    changeLogTab('trace');
                  }}
                  onOpenResumeTimeline={openResumeTimeline}
                  runId={selectedRunId}
                />
              )}
            </ApplicationLogsFloatingWindow>
          ) : null}
        </>
      )}
    </div>
  );
}
