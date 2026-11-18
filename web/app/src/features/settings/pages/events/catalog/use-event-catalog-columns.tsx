import type {
  EventCatalogEntry,
  EventCatalogQuery,
  EventTriggerPhase
} from '@1flowbase/api-client';
import { Button, Switch, Tag } from 'antd';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
import { formatDateTime } from '../../../../../shared/i18n/format';
import type { DataTableColumn } from '../../../../../shared/ui/data-table/DataTable';
import { DataTableRowActions } from '../../../../../shared/ui/data-table/DataTableRowActions';

export function useEventCatalogColumns({
  sort,
  onOpen,
  toggleEnabled
}: {
  sort: EventCatalogQuery | null;
  onOpen: (entry: EventCatalogEntry | null, mode?: 'view' | 'edit') => void;
  toggleEnabled: {
    isPending: boolean;
    variables?: { entry: EventCatalogEntry; enabled: boolean };
    mutate: (input: { entry: EventCatalogEntry; enabled: boolean }) => void;
  };
}): DataTableColumn<EventCatalogEntry>[] {
  const { t } = useTranslation('settingsEvents');
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const phase = (value: EventTriggerPhase) =>
    ({
      subscribed: t('subscribed'),
      published: t('published'),
      consumed: t('consumed')
    })[value];
  return [
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
          onClick={() => onOpen(row)}
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
      key: 'event_kind',
      title: t('event_kind'),
      width: 160,
      render: (_, row) =>
        row.event_kind === 'communication_command'
          ? t('communication_command_event')
          : row.event_kind === 'interface'
            ? t('interface_event')
            : '—'
    },
    {
      key: 'enabled',
      title: t('enabled'),
      width: 120,
      render: (_, row) => (
        <Switch
          aria-label={t('enabled')}
          checked={row.enabled !== false}
          disabled={
            row.origin !== 'custom' ||
            !row.id ||
            (!row.interface_event &&
              row.event_kind !== 'communication_command') ||
            row.supported_phases.length === 0 ||
            !csrfToken ||
            !can('events.definitions.update') ||
            toggleEnabled.isPending
          }
          loading={
            toggleEnabled.isPending &&
            toggleEnabled.variables?.entry.id === row.id
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
      key: 'updated_at',
      title: t('updated_at'),
      dataIndex: 'updated_at',
      width: 190,
      sorter: true,
      sortOrder:
        sort?.sort_by === 'updated_at'
          ? sort.sort_order === 'asc'
            ? 'ascend'
            : 'descend'
          : null,
      render: (_, row) =>
        row.updated_at ? formatDateTime(row.updated_at) : '—'
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
            (row.interface_event ||
              row.event_kind === 'communication_command') &&
            row.id &&
            row.supported_phases.length > 0 &&
            can('events.definitions.update')
              ? [
                  {
                    key: 'edit',
                    label: t('edit'),
                    onClick: () => onOpen(row, 'edit')
                  }
                ]
              : []),
            { key: 'view', label: t('view'), onClick: () => onOpen(row) }
          ]}
        />
      )
    }
  ];
}
