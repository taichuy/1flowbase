import type { EventWorkflowStatus } from '@1flowbase/api-client';
import { Typography } from 'antd';
import { RuntimeIdentity } from './runtime/RuntimeIdentity';
import { useTranslation } from 'react-i18next';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { EventsTable } from './EventsTable';
import { useWorkflowStatusLabels } from './runtime/runtime-labels';

export function EventWorkflowSubscriptions({
  subscriptions
}: {
  subscriptions: EventWorkflowStatus[];
}) {
  const { t } = useTranslation('settingsEvents');
  const { activation, reason } = useWorkflowStatusLabels();
  const rows = subscriptions.filter(
    (subscription) => subscription.activation !== 'active'
  );
  const columns: DataTableColumn<EventWorkflowStatus>[] = [
    {
      key: 'subscription_id',
      title: t('subscription_id'),
      render: (_, row) => (
        <RuntimeIdentity
          value={row.subscription_id}
          label={t('subscription_id')}
        />
      ),
      width: 240,
      mobileRole: 'title'
    },
    {
      key: 'application_id',
      title: t('application_id'),
      render: (_, row) => (
        <RuntimeIdentity
          value={row.application_id}
          label={t('application_id')}
        />
      ),
      width: 240
    },
    {
      key: 'activation',
      title: t('workflow_activation'),
      width: 180,
      render: (_, row) => activation(row.activation)
    },
    {
      key: 'reason',
      title: t('workflow_reason'),
      width: 320,
      render: (_, row) => reason(row.reason)
    }
  ];
  return rows.length > 0 ? (
    <>
      <Typography.Title level={5}>
        {t('inactive_workflow_subscriptions')}
      </Typography.Title>
      <EventsTable
        tableKey="workflow-subscriptions"
        columns={columns}
        rows={rows}
        rowKey="subscription_id"
      />
    </>
  ) : null;
}
