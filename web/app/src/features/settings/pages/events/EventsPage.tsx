import {
  createEventDefinition,
  listEventCatalog,
  updateEventDefinition,
  type CreateEventDefinitionInput,
  type EventCatalogEntry,
  type EventTriggerPhase
} from '@1flowbase/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Space, Tabs, Tag, Typography } from 'antd';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { DataTableRowActions } from '../../../../shared/ui/data-table/DataTableRowActions';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { EventDefinitionDrawer } from './EventDefinitionDrawer';
import { EventRuntimePanel } from './EventRuntimePanel';
import { EventsTable } from './EventsTable';

export function EventsPage() {
  const { t } = useTranslation('settingsEvents');
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [tab, setTab] = useState('catalog');
  const [drawer, setDrawer] = useState<{
    entry: EventCatalogEntry | null;
  } | null>(null);
  const canList = can('events.catalog.list');
  const canObserve = can('events.runtime.get');
  const catalog = useQuery({
    queryKey: ['settings', 'events', 'catalog'],
    queryFn: () => listEventCatalog(),
    enabled: canList
  });
  const editable = drawer?.entry
    ? drawer.entry.origin === 'custom' &&
      drawer.entry.id !== null &&
      drawer.entry.payload_schema !== null &&
      can('events.definitions.update')
    : can('events.definitions.create');
  const save = useMutation({
    mutationFn: async (values: CreateEventDefinitionInput) => {
      if (!csrfToken || !editable) throw new Error('operation not permitted');
      if (drawer?.entry?.id) {
        const { name, description, payload_schema, enabled } = values;
        return updateEventDefinition(
          drawer.entry.id,
          { name, description, payload_schema, enabled },
          csrfToken
        );
      }
      return createEventDefinition(values, csrfToken);
    },
    onSuccess: async () => {
      setDrawer(null);
      await queryClient.invalidateQueries({
        queryKey: ['settings', 'events', 'catalog']
      });
    }
  });
  const open = (entry: EventCatalogEntry | null) => {
    save.reset();
    setDrawer({ entry });
  };
  const phase = (value: EventTriggerPhase) =>
    ({
      subscribed: t('subscribed'),
      published: t('published'),
      consumed: t('consumed')
    })[value];
  const columns: DataTableColumn<EventCatalogEntry>[] = [
    {
      key: 'name',
      title: t('name'),
      width: 220,
      mobileRole: 'title',
      render: (_, row) => (
        <Button type="link" onClick={() => open(row)}>
          {row.name}
        </Button>
      )
    },
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 240
    },
    {
      key: 'contract_version',
      title: t('contract_version'),
      dataIndex: 'contract_version',
      width: 110
    },
    {
      key: 'origin',
      title: t('origin'),
      width: 120,
      render: (_, row) => (
        <Tag>
          {
            { custom: t('custom'), core: t('core'), plugin: t('plugin') }[
              row.origin
            ]
          }
        </Tag>
      )
    },
    {
      key: 'owner_id',
      title: t('owner_id'),
      dataIndex: 'owner_id',
      width: 220
    },
    {
      key: 'enabled',
      title: t('enabled'),
      width: 120,
      render: (_, row) => (row.enabled ? t('enabled') : t('disabled'))
    },
    {
      key: 'supported_phases',
      title: t('supported_phases'),
      width: 260,
      render: (_, row) =>
        row.supported_phases.length
          ? row.supported_phases.map(phase).join(' / ')
          : t('unsupported')
    },
    {
      key: 'actions',
      title: t('actions'),
      width: 120,
      mobileRole: 'actions',
      render: (_, row) => (
        <DataTableRowActions
          moreAriaLabel={t('actions')}
          moreItems={[{ key: 'view', label: t('view') }]}
          onMoreAction={() => open(row)}
        >
          {row.origin === 'custom' &&
            row.payload_schema !== null &&
            row.id &&
            can('events.definitions.update') && (
              <Button type="link" size="small" onClick={() => open(row)}>
                {t('edit')}
              </Button>
            )}
        </DataTableRowActions>
      )
    }
  ];
  return (
    <SettingsSectionSurface
      toolbar={
        <Typography.Title level={4}>
          {i18nText('settings', 'events.title')}
        </Typography.Title>
      }
    >
      <Tabs
        activeKey={tab}
        onChange={setTab}
        items={[
          {
            key: 'catalog',
            label: t('catalog'),
            children: canList ? (
              <Space
                orientation="vertical"
                size="middle"
                style={{ width: '100%' }}
              >
                <Space>
                  <Button
                    onClick={() => void catalog.refetch()}
                    loading={catalog.isFetching}
                  >
                    {t('refresh')}
                  </Button>
                  {can('events.definitions.create') && (
                    <Button type="primary" onClick={() => open(null)}>
                      {t('create')}
                    </Button>
                  )}
                </Space>
                {catalog.isError && (
                  <Alert type="error" showIcon title={t('catalog_error')} />
                )}
                <EventsTable
                  columns={columns}
                  rows={catalog.data ?? []}
                  rowKey={(row) =>
                    `${row.owner_id}:${row.contract_id}:${row.contract_version}`
                  }
                  tableKey="catalog"
                  loading={catalog.isPending}
                />
              </Space>
            ) : (
              <Alert type="warning" title={t('forbidden')} />
            )
          },
          ...(canObserve
            ? [
                {
                  key: 'queues',
                  label: t('queues'),
                  children:
                    tab === 'queues' ? <EventRuntimePanel tab="queues" /> : null
                },
                {
                  key: 'activity',
                  label: t('activity'),
                  children:
                    tab === 'activity' ? (
                      <EventRuntimePanel tab="activity" />
                    ) : null
                }
              ]
            : [])
        ]}
      />
      {drawer && (
        <EventDefinitionDrawer
          key={drawer.entry?.id ?? drawer.entry?.contract_id ?? 'create'}
          entry={drawer.entry}
          editable={editable}
          saving={save.isPending}
          error={save.isError}
          onClose={() => setDrawer(null)}
          onSave={(values) => save.mutate(values)}
        />
      )}
    </SettingsSectionSurface>
  );
}
