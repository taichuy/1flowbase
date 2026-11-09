import {
  createEventDefinition,
  eventCatalogIdentity,
  listEventCatalog,
  updateEventDefinition,
  type CreateEventDefinitionInput,
  type EventCatalogEntry,
  type EventTriggerPhase
} from '@1flowbase/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { Alert, Button, Flex, Switch, Tabs, Tag, TreeSelect } from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { DataTableRowActions } from '../../../../shared/ui/data-table/DataTableRowActions';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { EventDefinitionDrawer } from './EventDefinitionDrawer';
import { EventRuntimePanel } from './EventRuntimePanel';
import { EventsTable } from './EventsTable';
import {
  navigateEventTab,
  useEventTab,
  type EventTab
} from './event-navigation';
import { eventGroupTree } from './event-groups';

export function EventsPage() {
  const { t } = useTranslation('settingsEvents');
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const requestedTab = useEventTab();
  const [drawer, setDrawer] = useState<{
    entry: EventCatalogEntry | null;
    mode: 'view' | 'edit';
  } | null>(null);
  const [group, setGroup] = useState<string>();
  const canList = can('events.catalog.list');
  const canObserve = can('events.runtime.get');
  const tab = canObserve ? requestedTab : 'catalog';
  useEffect(() => {
    if (!canObserve && requestedTab !== 'catalog')
      navigateEventTab('catalog', true);
  }, [canObserve, requestedTab]);
  const catalog = useQuery({
    queryKey: ['events', 'catalog'],
    queryFn: () => listEventCatalog(),
    enabled: canList && tab === 'catalog'
  });
  const editable =
    drawer?.mode === 'edit' &&
    (drawer.entry
      ? drawer.entry.origin === 'custom' &&
        drawer.entry.id !== null &&
        !!drawer.entry.interface_event &&
        can('events.definitions.update')
      : can('events.definitions.create'));
  const save = useMutation({
    mutationFn: async (values: CreateEventDefinitionInput) => {
      if (!csrfToken || !editable) throw new Error('operation not permitted');
      if (drawer?.entry?.id) {
        const { name, description, interface_event, enabled } = values;
        return updateEventDefinition(
          drawer.entry.id,
          { name, description, interface_event, enabled },
          csrfToken
        );
      }
      return createEventDefinition(values, csrfToken);
    },
    onSuccess: async () => {
      setDrawer(null);
      await queryClient.invalidateQueries({
        queryKey: ['events', 'catalog']
      });
    }
  });
  const toggleEnabled = useMutation({
    mutationFn: async ({
      entry,
      enabled
    }: {
      entry: EventCatalogEntry;
      enabled: boolean;
    }) => {
      if (
        !csrfToken ||
        entry.origin !== 'custom' ||
        !entry.id ||
        !entry.interface_event ||
        !can('events.definitions.update')
      )
        throw new Error('operation not permitted');
      return updateEventDefinition(
        entry.id,
        {
          name: entry.name,
          description: entry.description,
          interface_event: entry.interface_event,
          enabled
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ['events', 'catalog']
      });
    }
  });
  const open = (
    entry: EventCatalogEntry | null,
    mode: 'view' | 'edit' = 'view'
  ) => {
    save.reset();
    setDrawer({ entry, mode });
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
      ellipsis: true,
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          title={row.name}
          styles={{
            root: { maxWidth: '100%' },
            content: {
              minWidth: 0,
              overflow: 'hidden',
              textOverflow: 'ellipsis'
            }
          }}
          onClick={() => open(row)}
        >
          {row.name}
        </Button>
      )
    },
    {
      key: 'group_path',
      title: t('group_path'),
      width: 220,
      render: (_, row) => row.group_path?.join(' / ') || '—'
    },
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 240
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
      key: 'scope',
      title: t('scope'),
      width: 220,
      render: (_, row) =>
        row.scope.kind === 'workspace'
          ? `${t('workspace_scope')} · ${row.scope.workspace_id}`
          : t(row.scope.kind)
    },
    {
      key: 'enabled',
      title: t('enabled'),
      width: 120,
      render: (_, row) => (
        <Switch
          aria-label={t('enabled')}
          checked={row.origin !== 'custom' || row.enabled === true}
          disabled={
            row.origin !== 'custom' ||
            !row.id ||
            !row.interface_event ||
            !csrfToken ||
            !can('events.definitions.update') ||
            toggleEnabled.isPending
          }
          loading={
            toggleEnabled.isPending &&
            toggleEnabled.variables.entry.id === row.id
          }
          onChange={(enabled) => toggleEnabled.mutate({ entry: row, enabled })}
        />
      )
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
      width: 180,
      mobileRole: 'actions',
      render: (_, row) => (
        <DataTableRowActions
          moreAriaLabel={t('actions')}
          actions={[
            ...(row.origin === 'custom' &&
            row.interface_event &&
            row.id &&
            can('events.definitions.update')
              ? [
                  {
                    key: 'edit',
                    label: t('edit'),
                    onClick: () => open(row, 'edit')
                  }
                ]
              : []),
            { key: 'view', label: t('view'), onClick: () => open(row) }
          ]}
        />
      )
    }
  ];
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16} style={{ height: '100%', minHeight: 0 }}>
        <Tabs
          activeKey={tab}
          onChange={(next) => navigateEventTab(next as EventTab)}
          items={[
            { key: 'catalog', label: t('catalog') },
            ...(canObserve
              ? [
                  { key: 'queues', label: t('queues') },
                  { key: 'activity', label: t('activity') }
                ]
              : [])
          ]}
        />
        {tab === 'catalog' ? (
          canList ? (
            <>
              {catalog.isError && (
                <Alert type="error" showIcon title={t('catalog_error')} />
              )}
              {toggleEnabled.isError && (
                <Alert type="error" showIcon title={t('save_error')} />
              )}
              <EventsTable
                columns={columns}
                rows={(catalog.data ?? []).filter(
                  (row) =>
                    !group ||
                    JSON.stringify(
                      row.group_path?.slice(
                        0,
                        (JSON.parse(group) as string[]).length
                      )
                    ) === group
                )}
                rowKey={(row) =>
                  JSON.stringify([eventCatalogIdentity(row), row.scope])
                }
                tableKey="catalog"
                loading={catalog.isPending}
                toolbar={
                  <>
                    <TreeSelect
                      aria-label={t('group_path')}
                      placeholder={t('all_groups')}
                      allowClear
                      value={group}
                      onChange={setGroup}
                      treeData={eventGroupTree(catalog.data ?? [])}
                      style={{ minWidth: 220 }}
                    />
                    {can('events.definitions.create') && (
                      <Button
                        type="primary"
                        aria-label={t('create')}
                        icon={<PlusOutlined />}
                        onClick={() => open(null, 'edit')}
                      >
                        {t('create')}
                      </Button>
                    )}
                    <Button
                      onClick={() => void catalog.refetch()}
                      loading={catalog.isFetching}
                    >
                      {t('refresh')}
                    </Button>
                  </>
                }
              />
            </>
          ) : (
            <Alert type="warning" title={t('forbidden')} />
          )
        ) : (
          <EventRuntimePanel key={tab} tab={tab} />
        )}
      </Flex>
      {drawer && (
        <EventDefinitionDrawer
          key={drawer.entry ? eventCatalogIdentity(drawer.entry) : 'create'}
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
