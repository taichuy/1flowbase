import { useMemo, useState } from 'react';

import { useQuery } from '@tanstack/react-query';
import { Button, Checkbox, Flex, Input, Select, Tag, Typography } from 'antd';
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
import './model-provider-request-logs-panel.css';
import {
  fetchSettingsModelProviderRequestLogs,
  settingsModelProviderRequestLogsQueryKey
} from '../../api/model-providers';

const PAGE_SIZE = 20;

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
  const [page, setPage] = useState(1);
  const [applicationName, setApplicationName] = useState('');
  const [providerInstanceId, setProviderInstanceId] = useState('');
  const [modelId, setModelId] = useState('');
  const [status, setStatus] = useState<string>();
  const [zeroOutputOnly, setZeroOutputOnly] = useState(false);
  const filter = useMemo<ConsoleModelProviderRequestLogsFilter>(
    () => ({
      page,
      page_size: PAGE_SIZE,
      application_name: applicationName.trim() || undefined,
      provider_instance_id: providerInstanceId.trim() || undefined,
      model_id: modelId.trim() || undefined,
      status,
      zero_output_only: zeroOutputOnly || undefined
    }),
    [applicationName, modelId, page, providerInstanceId, status, zeroOutputOnly]
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
        render: (value: string) => new Date(value).toLocaleString()
      },
      {
        key: 'application_name',
        title: i18nText('settings', 'auto.request_log_application'),
        dataIndex: 'application_name',
        width: 160
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
        render: (value: string | null) => value ?? '—'
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
        render: (value: number | null) => (
          <span
            style={{
              color: value === 0 ? 'var(--ant-color-error)' : undefined
            }}
          >
            {value ?? '—'}
          </span>
        )
      },
      {
        key: 'time_to_first_token_ms',
        title: i18nText('settings', 'auto.request_log_first_token'),
        dataIndex: 'time_to_first_token_ms',
        width: 110,
        render: formatDuration
      },
      {
        key: 'total_duration_ms',
        title: i18nText('settings', 'auto.request_log_total_duration'),
        dataIndex: 'total_duration_ms',
        width: 110,
        render: formatDuration
      },
      {
        key: 'attempt_index',
        title: i18nText('settings', 'auto.request_log_attempt'),
        dataIndex: 'attempt_index',
        width: 90
      },
      {
        key: 'flow_run_id',
        title: i18nText('settings', 'auto.request_log_run_id'),
        dataIndex: 'flow_run_id',
        width: 220,
        ellipsis: true,
        render: (value: string) => (
          <Typography.Text copyable>{value}</Typography.Text>
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

  return (
    <section className="model-provider-request-logs-panel">
      <div className="model-provider-request-logs-panel__toolbar">
        <Flex
          className="model-provider-request-logs-panel__filters"
          gap={12}
          wrap
        >
          <Input
            aria-label={i18nText('settings', 'auto.request_log_application')}
            placeholder={i18nText('settings', 'auto.request_log_application')}
            value={applicationName}
            onChange={(event) => {
              setPage(1);
              setApplicationName(event.target.value);
            }}
            style={{ width: 240 }}
          />
          <Input
            aria-label={i18nText('settings', 'auto.provider_instance_id')}
            placeholder={i18nText('settings', 'auto.provider_instance_id')}
            value={providerInstanceId}
            onChange={(event) => {
              setPage(1);
              setProviderInstanceId(event.target.value);
            }}
            style={{ width: 240 }}
          />
          <Input
            aria-label={i18nText('settings', 'auto.request_log_model')}
            placeholder={i18nText('settings', 'auto.request_log_model')}
            value={modelId}
            onChange={(event) => {
              setPage(1);
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
              setPage(1);
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
              setPage(1);
              setZeroOutputOnly(event.target.checked);
            }}
          >
            {i18nText('settings', 'auto.request_log_zero_output_only')}
          </Checkbox>
          <Button onClick={() => requestLogsQuery.refetch()}>
            {i18nText('settings', 'auto.refresh')}
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
          total={requestLogsQuery.data?.total_count ?? 0}
          onPageChange={setPage}
        />
      </div>
    </section>
  );
}
