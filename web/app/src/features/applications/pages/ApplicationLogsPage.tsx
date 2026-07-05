import {
  DownloadOutlined,
  ReloadOutlined,
  SearchOutlined,
  SortAscendingOutlined,
  SortDescendingOutlined,
  UploadOutlined
} from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Empty,
  Input,
  Progress,
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
  type Key
} from 'react';
import { useTranslation } from 'react-i18next';

import { AutosizeSelect } from '../../../shared/ui/autosize-select/AutosizeSelect';
import type { AgentFlowDebugMessage } from '../../agent-flow/api/runtime';
import { ConversationLogPanel } from '../../agent-flow/components/debug-console/ConversationLogPanel';
import {
  applicationRunsQueryKey,
  completeApplicationRunArchiveUploadSession,
  createApplicationRunArchiveUploadSession,
  fetchApplicationRunArchiveImportJob,
  fetchApplicationRuns,
  fetchApplicationRunTraceNodeChildren,
  fetchApplicationRunTraceNodeContent,
  fetchApplicationRunTraceNodeDetail,
  fetchApplicationRunTraceToolCallbackContent,
  fetchApplicationRunTraceTree,
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
} from '../api/runtime';
import { ApplicationRunDetailPanel } from '../components/logs/ApplicationRunDetailPanel';
import { ApplicationLogsFloatingWindow } from '../components/logs/ApplicationLogsFloatingWindow';
import { ApplicationRunResumeTimelinePanel } from '../components/logs/ApplicationRunResumeTimelinePanel';
import {
  clampRect,
  applyStoredWidth,
  DEFAULT_MIN_WIDTH,
  DEFAULT_MIN_HEIGHT,
  type FloatingWindowRect
} from '../components/logs/floating-window-geometry';
import { getApplicationRunsTableColumns } from '../components/logs/application-runs-table-columns';
import {
  ApplicationRunsTable,
  ApplicationRunsTableColumnSettings
} from '../components/logs/ApplicationRunsTable';
import { useApplicationRunsTableConfiguration } from '../components/logs/useApplicationRunsTableConfiguration';
import {
  buildRunTraceDumpFilename,
  buildSelectedRunTraceDumpFilename,
  saveApplicationRunExport
} from '../lib/run-export-download';
import { sha256ArrayBuffer } from '../lib/run-archive-hash';
import { useAuthStore } from '../../../state/auth-store';
import './application-logs-page.css';

const FLOATING_WINDOW_TOP = 112;
const FLOATING_WINDOW_GAP = 16;
const FLOATING_WINDOW_RIGHT = 32;
const FLOATING_WINDOW_MIN_WIDTH = 360;
const FLOATING_WINDOW_MAX_HEIGHT = 720;
const RUN_ARCHIVE_IMPORT_CHUNK_SIZE = 1024 * 1024;
const RUN_ARCHIVE_IMPORT_POLL_INTERVAL_MS = 1_000;
const RUN_ARCHIVE_IMPORT_MAX_POLLS = 120;
const DEFAULT_TIME_RANGE = '7';
const PAGE_SIZE = 20;

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

function getViewportSize() {
  if (typeof window === 'undefined') {
    return { width: 1280, height: 720 };
  }

  return {
    width: window.innerWidth,
    height: window.innerHeight
  };
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
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

function getFloatingWindowHeight() {
  const viewport = getViewportSize();

  return Math.max(
    320,
    Math.min(
      FLOATING_WINDOW_MAX_HEIGHT,
      viewport.height - FLOATING_WINDOW_TOP - FLOATING_WINDOW_RIGHT
    )
  );
}

function getRunDetailInitialRect() {
  const viewport = getViewportSize();

  return {
    left: viewport.width - FLOATING_WINDOW_MIN_WIDTH - FLOATING_WINDOW_RIGHT,
    top: FLOATING_WINDOW_TOP,
    width: FLOATING_WINDOW_MIN_WIDTH,
    height: getFloatingWindowHeight()
  };
}

function getConversationLogInitialRect() {
  const runDetailRect = getRunDetailInitialRect();

  return {
    left: runDetailRect.left - FLOATING_WINDOW_MIN_WIDTH - FLOATING_WINDOW_GAP,
    top: FLOATING_WINDOW_TOP,
    width: FLOATING_WINDOW_MIN_WIDTH,
    height: getFloatingWindowHeight()
  };
}

function getResumeTimelineInitialRect() {
  return getConversationLogInitialRect();
}

function resolveCollision(
  rectA: FloatingWindowRect,
  rectB: FloatingWindowRect,
  viewportWidth: number,
  minWidthB: number = DEFAULT_MIN_WIDTH,
  gap: number = FLOATING_WINDOW_GAP,
  margin: number = 8
): { rectA: FloatingWindowRect; rectB: FloatingWindowRect } {
  let nextLeftB = rectA.left - rectB.width - gap;

  if (nextLeftB < margin) {
    nextLeftB = margin;
    const availableWidthB = rectA.left - margin - gap;
    let nextWidthB = rectB.width;
    if (availableWidthB < rectB.width) {
      nextWidthB = Math.max(minWidthB, availableWidthB);
    }

    const overlap = nextLeftB + nextWidthB + gap - rectA.left;
    let nextLeftA = rectA.left;
    if (overlap > 0) {
      nextLeftA = Math.min(
        viewportWidth - rectA.width - margin,
        rectA.left + overlap
      );

      const newAvailableWidthB = nextLeftA - margin - gap;
      nextWidthB = Math.max(
        minWidthB,
        Math.min(rectB.width, newAvailableWidthB)
      );
      nextLeftB = Math.max(margin, nextLeftA - nextWidthB - gap);
    }

    return {
      rectA: { ...rectA, left: nextLeftA },
      rectB: { ...rectB, left: nextLeftB, width: nextWidthB }
    };
  } else {
    return {
      rectA,
      rectB: { ...rectB, left: nextLeftB }
    };
  }
}

export function ApplicationLogsPage({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [openConversationLogMessage, setOpenConversationLogMessage] =
    useState<AgentFlowDebugMessage | null>(null);
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
        readPersistedArchiveImportJob(applicationId)
      )
    );
  const [exportingRunId, setExportingRunId] = useState<string | null>(null);
  const [selectedRunIds, setSelectedRunIds] = useState<string[]>([]);
  const [activeFloatingWindow, setActiveFloatingWindow] =
    useState<ApplicationLogsFloatingWindowKind>('run-detail');
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
  const runsTableColumns = useMemo(
    () => getApplicationRunsTableColumns(t),
    [t]
  );
  const runsTableConfiguration =
    useApplicationRunsTableConfiguration(runsTableColumns);
  const titleIncludes = keywordSearch.trim();
  const runsInput: FetchApplicationRunsInput = useMemo(
    () => ({
      page,
      pageSize: PAGE_SIZE,
      timeRangeDays: timeRange === 'all' ? null : Number(timeRange),
      sortBy,
      sortOrder,
      titleIncludes: titleIncludes || undefined
    }),
    [page, sortBy, sortOrder, timeRange, titleIncludes]
  );
  const runsQuery = useQuery({
    queryKey: applicationRunsQueryKey(applicationId, runsInput),
    queryFn: () => fetchApplicationRuns(applicationId, runsInput)
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

  function selectRun(run: ApplicationRunSummary | null) {
    const nextRunId = run ? run.id : null;
    setSelectedRunId(nextRunId);
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
      const refreshedRuns = await fetchApplicationRuns(applicationId, {
        ...runsInput,
        cacheMode: 'refresh'
      });
      queryClient.setQueryData(
        applicationRunsQueryKey(applicationId, runsInput),
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
      const download = await exportSelectedApplicationRunsTraceDumpZip(
        applicationId,
        runIds,
        csrfToken
      );
      saveApplicationRunExport(download, buildSelectedRunTraceDumpFilename());
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
          applicationId,
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
    [applicationId]
  );

  const finishArchiveImportJob = useCallback(
    async (job: ApplicationRunArchiveImportJob | null) => {
      if (job && (job.status === 'succeeded' || job.status === 'failed')) {
        clearPersistedArchiveImportJob(applicationId);
      }

      if (!job || job.status !== 'succeeded') {
        message.error(t('auto.import_run_archive_failed'));
        return;
      }

      await queryClient.invalidateQueries({
        queryKey: applicationRunsQueryKey(applicationId, runsInput)
      });
      const targetRunId =
        job.source_to_target_run_ids[0]?.target_run_id ?? null;
      if (targetRunId) {
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
    [applicationId, message, queryClient, runsInput, t]
  );

  useEffect(() => {
    const persistedJob = readPersistedArchiveImportJob(applicationId);
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
  }, [applicationId, finishArchiveImportJob, waitForArchiveImportJob]);

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
        applicationId,
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
          applicationId,
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
        applicationId,
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
      writePersistedArchiveImportJob(applicationId, {
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
        applicationId,
        runId
      );
      saveApplicationRunExport(download, buildRunTraceDumpFilename(runId));
    } catch {
      message.error(t('auto.export_logs_failed'));
    } finally {
      setExportingRunId(null);
    }
  }

  function openConversationLog(message: AgentFlowDebugMessage) {
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

  const logsHeader = (
    <div className="application-logs-page__header">
      <div className="application-logs-page__filters" role="search">
        <AutosizeSelect<ApplicationLogTimeRange>
          aria-label={t('auto.time_range')}
          options={timeRangeOptions}
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
              disabled={archiveImportState !== null}
              icon={<DownloadOutlined aria-hidden="true" />}
              onClick={() => archiveImportInputRef.current?.click()}
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
          message={t('auto.logs_load_failed')}
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

  return (
    <div className="application-logs-page" data-testid="application-logs-page">
      <div className="application-logs-page__stack">
        {logsHeader}
        {archiveImportStatus}
        {logsList}
      </div>
      {openConversationLogMessage ? (
        <ApplicationLogsFloatingWindow
          active={activeFloatingWindow === 'conversation-log'}
          initialRect={getConversationLogInitialRect}
          rect={conversationLogRect ?? undefined}
          onRectChange={(rect) => handleRectChange('conversation-log', rect)}
          testId="application-logs-floating-conversation-log"
          title={t('auto.conversation_logs')}
          onActivate={() => setActiveFloatingWindow('conversation-log')}
        >
          <div className="application-logs-page__conversation-log-panel">
            <ConversationLogPanel
              defaultTraceToolsExpanded
              message={openConversationLogMessage}
              onClose={() => {
                setOpenConversationLogMessage(null);
                setConversationLogRect(null);
              }}
              onLoadArtifact={(artifactRef) =>
                fetchRuntimeDebugArtifact(applicationId, artifactRef)
              }
              onLoadArtifacts={(artifactRefs) =>
                fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
              }
              traceLoader={{
                loadTree: (runId) =>
                  fetchApplicationRunTraceTree(applicationId, runId),
                loadChildren: (runId, traceNodeId, cursor) =>
                  fetchApplicationRunTraceNodeChildren(
                    applicationId,
                    runId,
                    traceNodeId,
                    cursor
                  ),
                loadContent: (runId, traceNodeId) =>
                  fetchApplicationRunTraceNodeContent(
                    applicationId,
                    runId,
                    traceNodeId
                  ),
                loadDetail: (runId, traceNodeId, detailRefId) =>
                  fetchApplicationRunTraceNodeDetail(
                    applicationId,
                    runId,
                    traceNodeId,
                    detailRefId
                  ),
                loadToolCallbackDetail: (runId, traceNodeId, toolCallId) =>
                  fetchApplicationRunTraceToolCallbackContent(
                    applicationId,
                    runId,
                    traceNodeId,
                    toolCallId
                  )
              }}
              overviewLoader={{
                loadOverview: (runId) =>
                  fetchApplicationRunOverview(applicationId, runId)
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
              applicationId={applicationId}
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
          <ApplicationRunDetailPanel
            applicationId={applicationId}
            onClose={() => selectRun(null)}
            onOpenMessageLog={openConversationLog}
            onOpenResumeTimeline={openResumeTimeline}
            runId={selectedRunId}
          />
        </ApplicationLogsFloatingWindow>
      ) : null}
    </div>
  );
}
