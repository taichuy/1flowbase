import { useMemo, useState } from 'react';

import { useQuery } from '@tanstack/react-query';
import {
  Button,
  Checkbox,
  Flex,
  Input,
  Select,
  Space,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type {
  ConsoleModelProviderRequestLog,
  ConsoleModelProviderRequestLogsFilter
} from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
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
  const [applicationId, setApplicationId] = useState('');
  const [providerInstanceId, setProviderInstanceId] = useState('');
  const [modelId, setModelId] = useState('');
  const [status, setStatus] = useState<string>();
  const [zeroOutputOnly, setZeroOutputOnly] = useState(false);
  const filter = useMemo<ConsoleModelProviderRequestLogsFilter>(
    () => ({
      page,
      page_size: PAGE_SIZE,
      application_id: applicationId.trim() || undefined,
      provider_instance_id: providerInstanceId.trim() || undefined,
      model_id: modelId.trim() || undefined,
      status,
      zero_output_only: zeroOutputOnly || undefined
    }),
    [applicationId, modelId, page, providerInstanceId, status, zeroOutputOnly]
  );
  const requestLogsQuery = useQuery({
    queryKey: settingsModelProviderRequestLogsQueryKey(filter),
    queryFn: () => fetchSettingsModelProviderRequestLogs(filter)
  });
  const columns = useMemo<ColumnsType<ConsoleModelProviderRequestLog>>(
    () => [
      {
        title: i18nText('settings', 'auto.request_log_start_time'),
        dataIndex: 'started_at',
        width: 180,
        render: (value: string) => new Date(value).toLocaleString()
      },
      {
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
        title: i18nText('settings', 'auto.request_log_model'),
        dataIndex: 'upstream_model_id',
        width: 160
      },
      {
        title: i18nText('settings', 'auto.request_log_reasoning_effort'),
        dataIndex: 'reasoning_effort',
        width: 110,
        render: (value: string | null) => value ?? '—'
      },
      {
        title: i18nText('settings', 'auto.request_log_input_tokens'),
        dataIndex: 'input_tokens',
        width: 120
      },
      {
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
        title: i18nText('settings', 'auto.request_log_first_token'),
        dataIndex: 'time_to_first_token_ms',
        width: 110,
        render: formatDuration
      },
      {
        title: i18nText('settings', 'auto.request_log_total_duration'),
        dataIndex: 'total_duration_ms',
        width: 110,
        render: formatDuration
      },
      {
        title: i18nText('settings', 'auto.request_log_attempt'),
        dataIndex: 'attempt_index',
        width: 90,
        render: (value: number) => value + 1
      },
      {
        title: i18nText('settings', 'auto.request_log_run_id'),
        dataIndex: 'flow_run_id',
        width: 220,
        ellipsis: true,
        render: (value: string, row) => (
          <Typography.Link
            href={`/applications/${row.application_id}/logs?run_id=${value}`}
          >
            {value}
          </Typography.Link>
        )
      }
    ],
    []
  );

  return (
    <Space direction="vertical" size="middle" style={{ width: '100%' }}>
      <Flex gap={12} wrap>
        <Input
          aria-label={i18nText('settings', 'auto.request_log_application_id')}
          placeholder={i18nText('settings', 'auto.request_log_application_id')}
          value={applicationId}
          onChange={(event) => {
            setPage(1);
            setApplicationId(event.target.value);
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
      </Flex>
      <Table
        rowKey="attempt_id"
        columns={columns}
        dataSource={requestLogsQuery.data?.items ?? []}
        loading={requestLogsQuery.isLoading || requestLogsQuery.isFetching}
        scroll={{ x: 1680 }}
        pagination={{
          current: page,
          pageSize: PAGE_SIZE,
          total: requestLogsQuery.data?.total_count ?? 0,
          showSizeChanger: false,
          onChange: setPage
        }}
      />
    </Space>
  );
}
