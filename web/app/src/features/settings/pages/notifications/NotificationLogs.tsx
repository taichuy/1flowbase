import { useCallback, useMemo, useState, type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  DatePicker,
  Input,
  Tag,
  Empty,
  Flex,
  Select,
  Spin
} from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  NotificationLogListInput,
  NotificationSendLog
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../state/navigation-query-scope';
import { formatDateTime } from '../../../../shared/i18n/format';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  DataTable,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  notificationLogsOptions,
  notificationLogOptions,
  notificationInstancesOptions,
  notificationTypesOptions
} from '../../../notifications/api/queries';
import { useNotificationTypeLocalization } from '../../../notifications/hooks/use-notification-type-localization';

export function NotificationLogs() {
  const { t } = useTranslation('notifications');
  const localizeType = useNotificationTypeLocalization();
  const scope = useAuthStore(selectNavigationQueryScope);
  const [filters, setFilters] = useState<NotificationLogListInput>({});
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<string>();
  const logs = useQuery(
    notificationLogsOptions(scope, { ...filters, page, page_size: 20 })
  );
  const instances = useQuery(notificationInstancesOptions(scope));
  const types = useQuery(notificationTypesOptions(scope));
  const typeLabel = useCallback(
    (item: NotificationSendLog) => {
      const instance = instances.data?.find(
        (entry) => entry.id === item.instance_id
      );
      const type = types.data?.find(
        (entry) =>
          entry.type_code === item.type_code &&
          entry.installation_id === instance?.installation_id
      );
      return type
        ? localizeType(type).display_name
        : item.type_code === 'system'
          ? t('system_notification_type')
          : (item.type_code ?? '—');
    },
    [instances.data, types.data, localizeType, t]
  );
  const statusLabel = useCallback(
    (status: NotificationSendLog['status']) =>
      status === 'success'
        ? t('send_success')
        : status === 'failure'
          ? t('send_failure')
          : t('result_unconfirmed'),
    [t]
  );
  const failureLabel = useCallback(
    (code: string | null) => {
      switch (code) {
        case null:
          return '—';
        case 'instance_missing':
          return t('failure_instance_missing');
        case 'instance_disabled':
          return t('instance_disabled');
        case 'type_unavailable':
          return t('type_unavailable');
        case 'plugin_unavailable':
          return t('plugin_unavailable');
        case 'input_invalid':
          return t('failure_input_invalid');
        case 'config_invalid':
          return t('failure_config_invalid');
        case 'authorization_required':
          return t('failure_authorization_required');
        case 'authorization_invalid':
          return t('failure_authorization_invalid');
        case 'subscription_required':
          return t('failure_subscription_required');
        case 'subscription_expired':
          return t('failure_subscription_expired');
        case 'subscription_unavailable':
          return t('failure_subscription_unavailable');
        case 'recipient_not_ready':
          return t('failure_recipient_not_ready');
        case 'provider_rejected':
          return t('failure_provider_rejected');
        case 'provider_result_unknown':
          return t('failure_provider_result_unknown');
        case 'provider_output_invalid':
          return t('failure_provider_output_invalid');
        default:
          return t('failure_reason_unavailable');
      }
    },
    [t]
  );
  const change = (value: NotificationLogListInput) => {
    setFilters(value);
    setPage(1);
  };
  const columns = useMemo<Array<DataTableColumn<NotificationSendLog>>>(
    () => [
      {
        key: 'created_at',
        title: t('sent_at'),
        width: 190,
        render: (_, item) => formatDateTime(item.created_at)
      },
      {
        key: 'source',
        title: t('source'),
        width: 120,
        render: (_, item) =>
          item.source === 'workflow' ? t('source_workflow') : t('source_api')
      },
      {
        key: 'display_name',
        title: t('display_name'),
        width: 180,
        render: (_, item) => item.display_name ?? t('deleted_channel')
      },
      {
        key: 'type_code',
        title: t('type'),
        width: 140,
        dataIndex: 'type_code',
        render: (_, item) => typeLabel(item)
      },
      {
        key: 'status',
        title: t('send_status'),
        width: 140,
        render: (_, item) => statusLabel(item.status)
      },
      {
        key: 'failure_code',
        title: t('failure_reason'),
        width: 220,
        render: (_, item) => failureLabel(item.failure_code)
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 100,
        mobileRole: 'actions',
        render: (_, item) => (
          <Button type="link" onClick={() => setSelected(item.id)}>
            {t('detail')}
          </Button>
        )
      }
    ],
    [t, statusLabel, failureLabel, typeLabel]
  );
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'notifications.logs.columns'
  });
  return (
    <section className="notifications-page">
      {logs.isError && <Alert type="error" showIcon title={t('logs_failed')} />}
      <DataTable<NotificationSendLog>
        className="notifications-page__table"
        columns={columns}
        configuration={configuration}
        rowKey="id"
        dataSource={logs.data?.items ?? []}
        loading={logs.isPending}
        page={logs.data?.page ?? page}
        pageSize={logs.data?.page_size ?? 20}
        total={logs.data?.total ?? 0}
        onPageChange={setPage}
        emptyText={<Empty description={t('no_logs')} />}
        toolbar={
          <Flex gap="small" wrap>
            <Select
              className="notification-logs__filter"
              allowClear
              aria-label={t('source')}
              placeholder={t('source')}
              value={filters.source}
              options={[
                { value: 'api', label: t('source_api') },
                { value: 'workflow', label: t('source_workflow') }
              ]}
              onChange={(source) => change({ ...filters, source })}
            />
            <Select
              className="notification-logs__filter"
              allowClear
              aria-label={t('send_status')}
              placeholder={t('send_status')}
              value={filters.status}
              options={[
                { value: 'success', label: t('send_success') },
                { value: 'failure', label: t('send_failure') },
                { value: 'pending', label: t('status_pending') },
                { value: 'unknown', label: t('status_unknown') }
              ]}
              onChange={(status) => change({ ...filters, status })}
            />
            <Select
              className="notification-logs__filter"
              allowClear
              aria-label={t('display_name')}
              placeholder={t('display_name')}
              value={filters.instance_id}
              options={instances.data?.map((item) => ({
                value: item.id,
                label: item.display_name
              }))}
              onChange={(instance_id) => change({ ...filters, instance_id })}
            />
            <Select
              className="notification-logs__filter"
              allowClear
              aria-label={t('type')}
              placeholder={t('type')}
              value={filters.type_code}
              options={Array.from(
                new Map(
                  types.data?.map((item) => [
                    item.type_code,
                    {
                      value: item.type_code,
                      label: localizeType(item).display_name
                    }
                  ])
                ).values()
              )}
              onChange={(type_code) => change({ ...filters, type_code })}
            />
            <DatePicker.RangePicker
              aria-label={t('sent_at')}
              showTime
              onChange={(range) =>
                change({
                  ...filters,
                  created_from: range?.[0]?.toISOString(),
                  created_to: range?.[1]?.toISOString()
                })
              }
            />
            <Button
              onClick={() => void logs.refetch()}
              loading={logs.isFetching}
            >
              {t('refresh')}
            </Button>
          </Flex>
        }
      />
      {selected && (
        <NotificationLogDetail
          id={selected}
          onClose={() => setSelected(undefined)}
          statusLabel={statusLabel}
          failureLabel={failureLabel}
          typeLabel={typeLabel}
        />
      )}
    </section>
  );
}
function NotificationLogDetail({
  id,
  onClose,
  statusLabel,
  failureLabel,
  typeLabel
}: {
  id: string;
  onClose: () => void;
  statusLabel: (status: NotificationSendLog['status']) => string;
  failureLabel: (code: string | null) => string;
  typeLabel: (item: NotificationSendLog) => string;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const detail = useQuery(notificationLogOptions(scope, id));
  const item = detail.data;
  return (
    <ResizableDrawer
      open
      title={t('log_detail')}
      onClose={onClose}
      resizeLabel={t('resize_drawer')}
      rootClassName="notification-log-detail"
    >
      {detail.isError && <Alert type="error" title={t('logs_failed')} />}
      {detail.isPending && <Spin />}
      {item && (
        <section
          className="notification-log-detail__form"
          aria-label={t('log_detail')}
        >
          <dl className="notification-log-detail__fields">
            <NotificationLogField label={t('log_id')}>
              <Input
                readOnly
                value={item.id}
                aria-label={t('log_id')}
                className="notification-log-detail__id"
              />
            </NotificationLogField>
            <NotificationLogField label={t('channel_identifier')}>
              {item.instance_id}
            </NotificationLogField>
            <NotificationLogField label={t('display_name')}>
              {item.display_name ?? t('deleted_channel')}
            </NotificationLogField>
            <NotificationLogField label={t('type')}>
              <Tag>{typeLabel(item)}</Tag>
            </NotificationLogField>
            <NotificationLogField label={t('source')}>
              {item.source === 'workflow'
                ? t('source_workflow')
                : t('source_api')}
            </NotificationLogField>
            <NotificationLogField label={t('send_status')}>
              <Tag
                color={
                  item.status === 'success'
                    ? 'success'
                    : item.status === 'failure'
                      ? 'error'
                      : undefined
                }
              >
                {statusLabel(item.status)}
              </Tag>
            </NotificationLogField>
            <NotificationLogField label={t('message')}>
              <div className="notification-log-detail__message">
                <div
                  className="notification-log-detail__message-part"
                  role="group"
                  aria-label={t('message_title')}
                >
                  <span className="notification-log-detail__message-label">
                    {t('message_title')}
                  </span>
                  <div className="notification-log-detail__text">
                    {item.title ?? '—'}
                  </div>
                </div>
                <div
                  className="notification-log-detail__message-part"
                  role="group"
                  aria-label={t('message_content')}
                >
                  <span className="notification-log-detail__message-label">
                    {t('message_content')}
                  </span>
                  <div className="notification-log-detail__text">
                    {item.content ?? '—'}
                  </div>
                </div>
              </div>
            </NotificationLogField>
            {Object.keys(item.extra).length > 0 && (
              <NotificationLogField label={t('message_extra')}>
                <div role="group" aria-label={t('message_extra')}>
                  <NotificationLogValue value={item.extra} />
                </div>
              </NotificationLogField>
            )}
            {(item.status === 'failure' || item.status === 'unknown') &&
              item.failure_code !== null && (
                <NotificationLogField label={t('failure_reason')}>
                  {failureLabel(item.failure_code)}
                </NotificationLogField>
              )}
            <NotificationLogField label={t('recipient_count')}>
              {item.recipient_count ?? '—'}
            </NotificationLogField>
            <NotificationLogField label={t('completed_at')}>
              {item.completed_at ? formatDateTime(item.completed_at) : '—'}
            </NotificationLogField>
            <NotificationLogField label={t('sent_at')}>
              {formatDateTime(item.created_at)}
            </NotificationLogField>
          </dl>
        </section>
      )}
    </ResizableDrawer>
  );
}

function NotificationLogValue({ value }: { value: unknown }) {
  const { t } = useTranslation('notifications');
  if (Array.isArray(value)) {
    return value.length ? (
      <ol className="notification-log-detail__extra-list">
        {value.map((entry, index) => (
          <li key={index}>
            <NotificationLogValue value={entry} />
          </li>
        ))}
      </ol>
    ) : (
      <>—</>
    );
  }
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value);
    return entries.length ? (
      <dl className="notification-log-detail__extra-fields">
        {entries.map(([key, entry]) => (
          <NotificationLogField key={key} label={key}>
            <NotificationLogValue value={entry} />
          </NotificationLogField>
        ))}
      </dl>
    ) : (
      <>—</>
    );
  }
  return (
    <span className="notification-log-detail__text">
      {typeof value === 'boolean'
        ? value
          ? t('log_value_true')
          : t('log_value_false')
        : value == null
          ? '—'
          : String(value)}
    </span>
  );
}

function NotificationLogField({
  label,
  children
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="notification-log-detail__field">
      <dt className="notification-log-detail__label">{label}</dt>
      <dd className="notification-log-detail__value">{children}</dd>
    </div>
  );
}
