import {
  DataTable,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import { Button, Flex, Space, Switch, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
import type {
  CommunicationCommandConfiguration,
  CommunicationCommandSetting,
  CommunicationCommandView
} from '@1flowbase/api-client';
import './communication-commands.css';

type CommandRow = {
  configuration: CommunicationCommandConfiguration;
  declaration: CommunicationCommandView | undefined;
};

export function CommunicationCommandsTable({
  value,
  commands,
  loading,
  disabled,
  onChange,
  onEdit,
  onDelete
}: {
  value: CommunicationCommandConfiguration[];
  commands: CommunicationCommandView[];
  loading: boolean;
  disabled: boolean;
  onChange: (setting: CommunicationCommandSetting) => void;
  onEdit: (command_id: string) => void;
  onDelete: (command_id: string) => void;
}) {
  const { t } = useTranslation('notifications');
  const can = useConsoleOperationAccess();
  const canWrite = can('communications.binding.update');
  const configurations = new Map(
    value.map((command) => [command.command_id, command])
  );
  const rows: CommandRow[] = commands
    .filter((command) => command.source_kind !== 'user')
    .flatMap((declaration) => {
      const configuration = configurations.get(declaration.command_id);
      return configuration ? [{ configuration, declaration }] : [];
    });
  for (const configuration of value) {
    if (
      configuration.kind === 'event' ||
      !commands.some(
        (command) => command.command_id === configuration.command_id
      )
    ) {
      rows.push({
        configuration,
        declaration: commands.find(
          (command) => command.command_id === configuration.command_id
        )
      });
    }
  }
  const descriptions = new Map([
    ['host/new', t('command_new_description')],
    ['host/model', t('command_model_description')],
    ['host/help', t('command_help_description')]
  ]);
  const reasons = new Map([
    ['application_unavailable', t('command_application_unavailable')],
    ['model_selection_unavailable', t('command_model_selection_unavailable')],
    ['no_models_available', t('command_no_models_available')],
    ['plugin_unavailable', t('command_plugin_unavailable')],
    ['command_not_installed', t('command_not_installed')],
    ['command_name_conflict', t('command_name_conflict')],
    ['duplicate_command_identity', t('command_duplicate_identity')],
    ['event_command_target_unavailable', t('event_commands.target_unavailable')]
  ]);
  const name = ({ configuration, declaration }: CommandRow) =>
    configuration.kind === 'event'
      ? configuration.name
      : (declaration?.name ?? configuration.command_id);
  const columns: DataTableColumn<CommandRow>[] = [
    {
      key: 'name',
      title: t('command'),
      width: 190,
      mobileRole: 'title',
      render: (_, row) => (
        <Typography.Text code>
          {row.configuration.kind === 'event'
            ? `/${row.configuration.name} [arguments]`
            : row.declaration?.usage || name(row)}
        </Typography.Text>
      )
    },
    {
      key: 'source_kind',
      title: t('command_source'),
      width: 130,
      render: (_, { configuration, declaration }) => (
        <Flex vertical gap={2}>
          <Typography.Text>
            {configuration.kind === 'event'
              ? t('event_commands.source_user')
              : declaration?.source_kind === 'builtin'
                ? t('command_source_builtin')
                : declaration?.source_kind === 'plugin'
                  ? t('command_source_plugin')
                  : '—'}
          </Typography.Text>
          {configuration.kind !== 'event' && declaration && (
            <Typography.Text type="secondary">
              {declaration.source_name}
            </Typography.Text>
          )}
        </Flex>
      )
    },
    {
      key: 'description',
      title: t('command_description'),
      width: 340,
      render: (_, { configuration, declaration }) => (
        <Flex vertical gap={4}>
          <Typography.Text>
            {configuration.kind === 'event'
              ? configuration.description
              : declaration?.source_kind === 'builtin'
                ? (descriptions.get(configuration.command_id) ??
                  declaration.description)
                : declaration?.description}
          </Typography.Text>
          {declaration?.available === false && (
            <Typography.Text type="secondary">
              {declaration.unavailable_reason
                ? t('command_unavailable_reason', {
                    reason:
                      reasons.get(declaration.unavailable_reason) ??
                      declaration.unavailable_reason
                  })
                : t('command_unavailable')}
            </Typography.Text>
          )}
        </Flex>
      )
    },
    {
      key: 'enabled',
      title: t('enable_status'),
      width: 86,
      align: 'center',
      render: (_, row) => (
        <span className="communication-commands__toggle">
          <Switch
            aria-label={t('command_toggle', { command: name(row) })}
            checked={row.configuration.enabled}
            disabled={
              !canWrite ||
              disabled ||
              (row.declaration?.available === false &&
                !row.configuration.enabled)
            }
            onChange={(enabled) =>
              canWrite &&
              onChange({ command_id: row.configuration.command_id, enabled })
            }
          />
        </span>
      )
    },
    {
      key: 'actions',
      title: t('actions'),
      width: 150,
      mobileRole: 'actions',
      render: (_, { configuration }) =>
        configuration.kind === 'event' ? (
          <Space>
            <Button
              type="link"
              disabled={!canWrite || disabled}
              onClick={() => onEdit(configuration.command_id)}
            >
              {t('edit')}
            </Button>
            <Button
              type="link"
              danger
              disabled={!canWrite || disabled}
              onClick={() => onDelete(configuration.command_id)}
            >
              {t('delete')}
            </Button>
          </Space>
        ) : null
    }
  ];
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'communications.commands'
  });
  return (
    <DataTable<CommandRow>
      className="communication-commands"
      rowKey={(row) =>
        `${row.configuration.command_id}:${row.declaration?.installation_id ?? ''}`
      }
      columns={columns}
      configuration={configuration}
      loading={loading}
      dataSource={rows}
      emptyText={loading ? null : t('no_commands')}
      page={1}
      pageSize={Math.max(rows.length, 1)}
      total={rows.length}
      onPageChange={() => {}}
    />
  );
}
