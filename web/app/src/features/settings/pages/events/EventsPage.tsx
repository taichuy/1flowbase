import {
  createEventDefinition,
  eventCatalogIdentity,
  listEventCatalog,
  updateEventDefinition,
  type CreateEventDefinitionInput,
  type EventCatalogEntry,
  type EventCatalogQuery
} from '@1flowbase/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { Alert, Button, Flex, Tabs, TreeSelect } from 'antd';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { useEventCatalogColumns } from './catalog/use-event-catalog-columns';
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

type DrawerSession = { entry: EventCatalogEntry | null; mode: 'view' | 'edit' };

export function EventsPage() {
  const { t } = useTranslation('settingsEvents');
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const requestedTab = useEventTab();
  const [drawer, setDrawer] = useState<DrawerSession | null>(null);
  const currentDrawer = useRef<DrawerSession | null>(null);
  useEffect(
    () => () => {
      currentDrawer.current = null;
    },
    []
  );
  const closeDrawer = () => {
    currentDrawer.current = null;
    setDrawer(null);
  };
  const [group, setGroup] = useState<string>();
  const [sort, setSort] = useState<EventCatalogQuery | null>({
    sort_by: 'updated_at',
    sort_order: 'desc'
  });
  const effectiveSort =
    sort ?? ({ sort_by: 'updated_at', sort_order: 'desc' } as const);
  const canList = can('events.catalog.list');
  const canObserve = can('events.runtime.get');
  const tab = canObserve ? requestedTab : 'catalog';
  useEffect(() => {
    if (!canObserve && requestedTab !== 'catalog')
      navigateEventTab('catalog', true);
  }, [canObserve, requestedTab]);
  const catalog = useQuery({
    queryKey: ['events', 'catalog', effectiveSort],
    queryFn: () => listEventCatalog(undefined, effectiveSort),
    enabled: canList && tab === 'catalog'
  });
  const editable =
    drawer?.mode === 'edit' &&
    (drawer.entry
      ? drawer.entry.origin === 'custom' &&
        drawer.entry.id !== null &&
        (!!drawer.entry.interface_event ||
          drawer.entry.event_kind === 'communication_command') &&
        drawer.entry.supported_phases.length > 0 &&
        can('events.definitions.update')
      : can('events.definitions.create'));
  const save = useMutation({
    mutationFn: async ({
      values,
      session
    }: {
      values: CreateEventDefinitionInput;
      session: DrawerSession;
    }) => {
      if (!csrfToken || !editable || currentDrawer.current !== session)
        throw new Error('operation not permitted');
      if (session.entry?.id) {
        return updateEventDefinition(session.entry.id, values, csrfToken);
      }
      return createEventDefinition(values, csrfToken);
    },
    onSuccess: async (_, { session }) => {
      if (currentDrawer.current === session) closeDrawer();
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
        (!entry.interface_event &&
          entry.event_kind !== 'communication_command') ||
        !can('events.definitions.update')
      )
        throw new Error('operation not permitted');
      return updateEventDefinition(
        entry.id,
        {
          name: entry.name,
          description: entry.description,
          interface_event: entry.interface_event,
          event_kind: entry.event_kind,
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
    const session = { entry, mode };
    currentDrawer.current = session;
    setDrawer(session);
  };
  const columns = useEventCatalogColumns({ sort, onOpen: open, toggleEnabled });
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex
        vertical
        gap={16}
        style={{
          height: tab === 'catalog' ? '100%' : 'max-content',
          minHeight: 0
        }}
      >
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
                onChange={(_, __, sorter) => {
                  const selected = Array.isArray(sorter) ? sorter[0] : sorter;
                  setSort(
                    selected?.order && selected.columnKey === 'updated_at'
                      ? {
                          sort_by: selected.columnKey,
                          sort_order:
                            selected.order === 'ascend' ? 'asc' : 'desc'
                        }
                      : null
                  );
                }}
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
          saving={save.isPending && save.variables.session === drawer}
          error={save.isError && save.variables.session === drawer}
          onClose={closeDrawer}
          onSave={(values) => save.mutate({ values, session: drawer })}
        />
      )}
    </SettingsSectionSurface>
  );
}
