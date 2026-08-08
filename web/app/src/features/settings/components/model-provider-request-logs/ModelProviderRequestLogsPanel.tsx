import { useMemo, useState } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Checkbox,
  Flex,
  Input,
  Modal,
  Select,
  Tag,
  Typography
} from 'antd';
import type {
  ConsoleModelProviderRequestLog,
  ConsoleModelProviderRequestLogsFilter
} from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import { useAuthStore } from '../../../../state/auth-store';
import './model-provider-request-logs-panel.css';
import {
  clearSettingsModelProviderRequestLogsBatch,
  deleteSettingsModelProviderRequestLogs,
  fetchSettingsModelProviderRequestLogs,
  settingsModelProviderRequestLogsQueryKey
} from '../../api/model-providers';

const PAGE_SIZE = 20;

type RequestLogTimeRange = 'today' | '7' | '28' | '90' | '365' | 'all';

type ClearProgress = {
  deletedCount: number;
  continuationToken?: string;
  status: 'running' | 'failed' | 'complete';
};

const DEFAULT_TIME_RANGE: RequestLogTimeRange = '7';

function startedAfterForRange(timeRange: RequestLogTimeRange) {
  if (timeRange === 'all') return undefined;
  const now = new Date();
  if (timeRange === 'today') {
    now.setHours(0, 0, 0, 0);
    return now.toISOString();
  }
  return new Date(
    now.getTime() - Number(timeRange) * 24 * 60 * 60 * 1000
  ).toISOString();
}

function formatDuration(value: number | null) {
  if (value === null) return '—';
  return value < 1000 ? `${value} ms` : `${(value / 1000).toFixed(2)} s`;
}

function statusTag(record: ConsoleModelProviderRequestLog) {
  const emptyResponse = record.status === 'empty_response';
  const color =
    record.status === 'succeeded'
      ? 'success'
      : emptyResponse
        ? 'warning'
        : 'error';
  const label = emptyResponse
    ? i18nText('settings', 'auto.request_log_empty_response')
    : record.status === 'failed_after_first_token'
      ? i18nText('settings', 'auto.request_log_failed_after_first_token')
      : record.status === 'failed'
        ? i18nText('settings', 'auto.request_log_failed')
        : i18nText('settings', 'auto.request_log_succeeded');
  return <Tag color={color}>{label}</Tag>;
}

export function ModelProviderRequestLogsPanel() {
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [page, setPage] = useState(1);
  const [timeRange, setTimeRange] =
    useState<RequestLogTimeRange>(DEFAULT_TIME_RANGE);
  const [userId, setUserId] = useState('');
  const [applicationName, setApplicationName] = useState('');
  const [providerInstanceId, setProviderInstanceId] = useState('');
  const [modelId, setModelId] = useState('');
  const [status, setStatus] = useState<string>();
  const [zeroOutputOnly, setZeroOutputOnly] = useState(false);
  const [selectedAttemptIds, setSelectedAttemptIds] = useState<string[]>([]);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [deletingSelected, setDeletingSelected] = useState(false);
  const [clearProgress, setClearProgress] = useState<ClearProgress | null>(
    null
  );
  const startedAfter = useMemo(
    () => startedAfterForRange(timeRange),
    [timeRange]
  );
  const filter = useMemo<ConsoleModelProviderRequestLogsFilter>(
    () => ({
      page,
      page_size: PAGE_SIZE,
      user_id: userId.trim() || undefined,
      application_name: applicationName.trim() || undefined,
      provider_instance_id: providerInstanceId.trim() || undefined,
      model_id: modelId.trim() || undefined,
      status,
      zero_output_only: zeroOutputOnly || undefined,
      started_after: startedAfter
    }),
    [
      applicationName,
      modelId,
      page,
      providerInstanceId,
      startedAfter,
      status,
      userId,
      zeroOutputOnly
    ]
  );
  const requestLogsQuery = useQuery({
    queryKey: settingsModelProviderRequestLogsQueryKey(filter),
    queryFn: () => fetchSettingsModelProviderRequestLogs(filter)
  });
  const columns = useMemo<
    Array<DataTableColumn<ConsoleModelProviderRequestLog>>
  >(
    () => [
      {
        key: 'started_at',
        title: i18nText('settings', 'auto.request_log_start_time'),
        dataIndex: 'started_at',
        width: 180,
        render: (value) =>
          typeof value === 'string' ? new Date(value).toLocaleString() : '—'
      },
      {
        key: 'application_name',
        title: i18nText('settings', 'auto.request_log_application'),
        dataIndex: 'application_name',
        width: 160
      },
      {
        key: 'user_account',
        title: i18nText('settings', 'auto.request_log_user_account'),
        dataIndex: 'user_account',
        width: 160,
        render: (value) => (typeof value === 'string' ? value : '—')
      },
      {
        key: 'user_id',
        title: i18nText('settings', 'auto.request_log_user_id'),
        dataIndex: 'user_id',
        width: 220,
        ellipsis: true,
        render: (value) => (typeof value === 'string' ? value : '—')
      },
      {
        title: i18nText('settings', 'auto.status'),
        key: 'status',
        width: 120,
        render: (_, row) => statusTag(row)
      },
      {
        title: i18nText('settings', 'auto.provider'),
        key: 'provider',
        width: 180,
        render: (_, row) =>
          row.provider_instance_display_name ?? row.provider_code
      },
      {
        key: 'upstream_model_id',
        title: i18nText('settings', 'auto.request_log_model'),
        dataIndex: 'upstream_model_id',
        width: 160
      },
      {
        key: 'reasoning_effort',
        title: i18nText('settings', 'auto.request_log_reasoning_effort'),
        dataIndex: 'reasoning_effort',
        width: 110,
        render: (value: unknown) => (typeof value === 'string' ? value : '—')
      },
      {
        key: 'input_tokens',
        title: i18nText('settings', 'auto.request_log_input_tokens'),
        dataIndex: 'input_tokens',
        width: 120
      },
      {
        key: 'output_tokens',
        title: i18nText('settings', 'auto.request_log_output_tokens'),
        dataIndex: 'output_tokens',
        width: 120,
        render: (value) => (
          <span
            style={{
              color: value === 0 ? 'var(--ant-color-error)' : undefined
            }}
          >
            {typeof value === 'number' ? value : '—'}
          </span>
        )
      },
      {
        key: 'time_to_first_token_ms',
        title: i18nText('settings', 'auto.request_log_first_token'),
        dataIndex: 'time_to_first_token_ms',
        width: 110,
        render: (value) =>
          formatDuration(typeof value === 'number' ? value : null)
      },
      {
        key: 'total_duration_ms',
        title: i18nText('settings', 'auto.request_log_total_duration'),
        dataIndex: 'total_duration_ms',
        width: 110,
        render: (value) =>
          formatDuration(typeof value === 'number' ? value : null)
      },
      {
        key: 'attempt_index',
        title: i18nText('settings', 'auto.request_log_request_sequence'),
        dataIndex: 'attempt_index',
        width: 100
      },
      {
        key: 'is_retry',
        title: i18nText('settings', 'auto.request_log_is_retry'),
        dataIndex: 'is_retry',
        width: 90,
        render: (value: unknown) =>
          value === true
            ? i18nText('settings', 'auto.yes')
            : i18nText('settings', 'auto.no')
      },
      {
        key: 'retry_reason',
        title: i18nText('settings', 'auto.request_log_retry_reason'),
        dataIndex: 'retry_reason',
        width: 160,
        ellipsis: true,
        render: (value) => (typeof value === 'string' ? value : '—')
      },
      {
        key: 'conversation_link',
        title: i18nText('settings', 'auto.request_log_conversation'),
        width: 120,
        render: (_, row) =>
          row.application_id && row.flow_run_id ? (
            <Typography.Link
              href={`/applications/${encodeURIComponent(row.application_id)}/logs?run_id=${encodeURIComponent(row.flow_run_id)}`}
            >
              {i18nText('settings', 'auto.request_log_view_conversation')}
            </Typography.Link>
          ) : (
            '—'
          )
      }
    ],
    []
  );
  const tableConfiguration =
    useUserPreferenceDataTableConfiguration<ConsoleModelProviderRequestLog>({
      columns,
      preferenceKey: 'settings.model_provider_request_logs'
    });

  function resetPageAndSelection() {
    setPage(1);
    setSelectedAttemptIds([]);
  }

  async function invalidateRequestLogs() {
    await queryClient.invalidateQueries({
      queryKey: ['settings', 'model-providers', 'request-logs']
    });
  }

  async function deleteSelectedRequestLogs() {
    if (!csrfToken || selectedAttemptIds.length === 0) return;
    setDeletingSelected(true);
    try {
      await deleteSettingsModelProviderRequestLogs(
        { attempt_ids: selectedAttemptIds },
        csrfToken
      );
      setDeleteConfirmOpen(false);
      setSelectedAttemptIds([]);
      await invalidateRequestLogs();
    } finally {
      setDeletingSelected(false);
    }
  }

  async function clearRequestLogs(
    continuationToken?: string,
    alreadyDeleted = 0
  ) {
    if (!csrfToken) return;
    setClearConfirmOpen(false);
    let continuation = continuationToken;
    let deletedCount = alreadyDeleted;
    setClearProgress({
      deletedCount,
      continuationToken: continuation,
      status: 'running'
    });
    try {
      while (true) {
        const batch = await clearSettingsModelProviderRequestLogsBatch(
          continuation ? { continuation_token: continuation } : {},
          csrfToken
        );
        continuation = batch.continuation_token;
        deletedCount += batch.deleted_count;
        setClearProgress({
          deletedCount,
          continuationToken: continuation,
          status: batch.has_more ? 'running' : 'complete'
        });
        if (!batch.has_more) {
          setSelectedAttemptIds([]);
          await invalidateRequestLogs();
          return;
        }
      }
    } catch {
      setClearProgress({
        deletedCount,
        continuationToken: continuation,
        status: 'failed'
      });
    }
  }

  return (
    <section className="model-provider-request-logs-panel">
      <div className="model-provider-request-logs-panel__toolbar">
        <Flex
          className="model-provider-request-logs-panel__filters"
          gap={12}
          wrap
        >
          <Select<RequestLogTimeRange>
            aria-label={i18nText('settings', 'auto.request_log_time_range')}
            value={timeRange}
            onChange={(value) => {
              resetPageAndSelection();
              setTimeRange(value);
            }}
            style={{ width: 150 }}
            options={[
              {
                label: i18nText('settings', 'auto.request_log_today'),
                value: 'today'
              },
              {
                label: i18nText('settings', 'auto.request_log_past_seven_days'),
                value: '7'
              },
              {
                label: i18nText('settings', 'auto.request_log_past_four_weeks'),
                value: '28'
              },
              {
                label: i18nText(
                  'settings',
                  'auto.request_log_past_three_months'
                ),
                value: '90'
              },
              {
                label: i18nText(
                  'settings',
                  'auto.request_log_past_twelve_months'
                ),
                value: '365'
              },
              {
                label: i18nText('settings', 'auto.request_log_all_time'),
                value: 'all'
              }
            ]}
          />
          <Input
            aria-label={i18nText('settings', 'auto.request_log_user_id')}
            placeholder={i18nText('settings', 'auto.request_log_user_id')}
            value={userId}
            onChange={(event) => {
              resetPageAndSelection();
              setUserId(event.target.value);
            }}
            style={{ width: 240 }}
          />
          <Input
            aria-label={i18nText('settings', 'auto.request_log_application')}
            placeholder={i18nText('settings', 'auto.request_log_application')}
            value={applicationName}
            onChange={(event) => {
              resetPageAndSelection();
              setApplicationName(event.target.value);
            }}
            style={{ width: 240 }}
          />
          <Input
            aria-label={i18nText('settings', 'auto.provider_instance_id')}
            placeholder={i18nText('settings', 'auto.provider_instance_id')}
            value={providerInstanceId}
            onChange={(event) => {
              resetPageAndSelection();
              setProviderInstanceId(event.target.value);
            }}
            style={{ width: 240 }}
          />
          <Input
            aria-label={i18nText('settings', 'auto.request_log_model')}
            placeholder={i18nText('settings', 'auto.request_log_model')}
            value={modelId}
            onChange={(event) => {
              resetPageAndSelection();
              setModelId(event.target.value);
            }}
            style={{ width: 180 }}
          />
          <Select
            aria-label={i18nText('settings', 'auto.status')}
            allowClear
            placeholder={i18nText('settings', 'auto.status')}
            value={status}
            onChange={(value) => {
              resetPageAndSelection();
              setStatus(value);
            }}
            style={{ width: 160 }}
            options={[
              {
                label: i18nText('settings', 'auto.request_log_succeeded'),
                value: 'succeeded'
              },
              {
                label: i18nText('settings', 'auto.request_log_empty_response'),
                value: 'empty_response'
              },
              {
                label: i18nText('settings', 'auto.request_log_failed'),
                value: 'failed'
              },
              {
                label: i18nText(
                  'settings',
                  'auto.request_log_failed_after_first_token'
                ),
                value: 'failed_after_first_token'
              }
            ]}
          />
        </Flex>
        <Flex
          className="model-provider-request-logs-panel__actions"
          gap={12}
          wrap
        >
          <Checkbox
            checked={zeroOutputOnly}
            onChange={(event) => {
              resetPageAndSelection();
              setZeroOutputOnly(event.target.checked);
            }}
          >
            {i18nText('settings', 'auto.request_log_zero_output_only')}
          </Checkbox>
          <Button onClick={() => requestLogsQuery.refetch()}>
            {i18nText('settings', 'auto.refresh')}
          </Button>
          <Button
            danger
            disabled={selectedAttemptIds.length === 0}
            loading={deletingSelected}
            onClick={() => setDeleteConfirmOpen(true)}
          >
            {i18nText('settings', 'auto.request_log_delete_selected')}
          </Button>
          <Button
            danger
            disabled={clearProgress?.status === 'running'}
            onClick={() => setClearConfirmOpen(true)}
          >
            {i18nText('settings', 'auto.request_log_clear')}
          </Button>
          <DataTableColumnSettings
            columns={columns}
            configuration={tableConfiguration}
          />
        </Flex>
      </div>
      <div className="model-provider-request-logs-panel__table-region">
        <DataTable<ConsoleModelProviderRequestLog>
          className="model-provider-request-logs-table"
          columns={columns}
          configuration={tableConfiguration}
          dataSource={requestLogsQuery.data?.items ?? []}
          loading={requestLogsQuery.isLoading || requestLogsQuery.isFetching}
          page={page}
          pageSize={PAGE_SIZE}
          rowKey="attempt_id"
          rowSelection={{
            selectedRowKeys: selectedAttemptIds,
            onChange: (keys) =>
              setSelectedAttemptIds(keys.map((key) => String(key))),
            getCheckboxProps: (record) => ({
              id: `request-log-selection-${record.attempt_id}`
            }),
            renderCell: (_checked, record, _index, originNode) => (
              <>
                {originNode}
                <label
                  htmlFor={`request-log-selection-${record.attempt_id}`}
                  style={{
                    border: 0,
                    clip: 'rect(0 0 0 0)',
                    height: 1,
                    margin: -1,
                    overflow: 'hidden',
                    padding: 0,
                    position: 'absolute',
                    whiteSpace: 'nowrap',
                    width: 1
                  }}
                >
                  {i18nText('settings', 'auto.request_log_select_record')}
                </label>
              </>
            )
          }}
          total={requestLogsQuery.data?.total_count ?? 0}
          onPageChange={(nextPage) => {
            setSelectedAttemptIds([]);
            setPage(nextPage);
          }}
        />
      </div>
      {clearProgress ? (
        <Alert
          action={
            clearProgress.status === 'failed' ? (
              <Button
                size="small"
                onClick={() =>
                  void clearRequestLogs(
                    clearProgress.continuationToken,
                    clearProgress.deletedCount
                  )
                }
              >
                {i18nText('settings', 'auto.request_log_retry_clear')}
              </Button>
            ) : undefined
          }
          title={
            clearProgress.status === 'failed'
              ? i18nText('settings', 'auto.request_log_clear_stopped', {
                  count: clearProgress.deletedCount
                })
              : i18nText('settings', 'auto.request_log_clear_progress', {
                  count: clearProgress.deletedCount
                })
          }
          role="status"
          showIcon
          type={clearProgress.status === 'failed' ? 'error' : 'info'}
        />
      ) : null}
      <Modal
        cancelText={i18nText('settings', 'auto.cancel')}
        confirmLoading={deletingSelected}
        okText={i18nText('settings', 'auto.confirm_delete')}
        open={deleteConfirmOpen}
        title={i18nText('settings', 'auto.request_log_delete_selected')}
        onCancel={() => setDeleteConfirmOpen(false)}
        onOk={() => void deleteSelectedRequestLogs()}
      >
        <Typography.Text>
          {i18nText('settings', 'auto.request_log_delete_selected_confirm', {
            count: selectedAttemptIds.length
          })}
        </Typography.Text>
      </Modal>
      <Modal
        cancelText={i18nText('settings', 'auto.cancel')}
        okButtonProps={{ danger: true }}
        okText={i18nText('settings', 'auto.request_log_confirm_clear')}
        open={clearConfirmOpen}
        title={i18nText('settings', 'auto.request_log_clear')}
        onCancel={() => setClearConfirmOpen(false)}
        onOk={() => void clearRequestLogs()}
      >
        <Typography.Text>
          {i18nText('settings', 'auto.request_log_clear_confirm')}
        </Typography.Text>
      </Modal>
    </section>
  );
}
