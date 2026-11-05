import {
  DataTable,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import { Flex, Switch, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
import type {
  CommunicationCommandSetting,
  CommunicationCommandView
} from '@1flowbase/api-client';
import './communication-commands.css';

export function CommunicationCommandsTable({
  commands,
  loading,
  disabled,
  onChange
}: {
  commands: CommunicationCommandView[];
  loading: boolean;
  disabled: boolean;
  onChange: (setting: CommunicationCommandSetting) => void;
}) {
  const { t } = useTranslation('notifications');
  const can = useConsoleOperationAccess();
  const canWrite = can('communications.binding.update');

  // Static translation calls keep built-in copy visible to i18n hygiene.
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
    ['duplicate_command_identity', t('command_duplicate_identity')]
  ]);
  const source = (command: CommunicationCommandView) => (
    <Flex vertical gap={2}>
      <Typography.Text>
        {command.source_kind === 'builtin'
          ? t('command_source_builtin')
          : command.source_kind === 'user'
            ? t('event_commands.source_user')
            : t('command_source_plugin')}
      </Typography.Text>
      <Typography.Text type="secondary">{command.source_name}</Typography.Text>
    </Flex>
  );
  const description = (command: CommunicationCommandView) => (
    <Flex vertical gap={4}>
      <Typography.Text>
        {command.source_kind === 'builtin'
          ? (descriptions.get(command.command_id) ?? command.description)
          : command.description}
      </Typography.Text>
      {!command.available && (
        <Typography.Text type="secondary">
          {command.unavailable_reason
            ? t('command_unavailable_reason', {
                reason:
                  reasons.get(command.unavailable_reason) ??
                  command.unavailable_reason
              })
            : t('command_unavailable')}
        </Typography.Text>
      )}
    </Flex>
  );
  const columns: DataTableColumn<CommunicationCommandView>[] = [
    {
      key: 'name',
      title: t('command'),
      width: 190,
      render: (_, command) => (
        <Flex vertical gap={8}>
          <Typography.Text code>
            {command.usage || command.name}
          </Typography.Text>
        </Flex>
      )
    },
    {
      key: 'source_kind',
      title: t('command_source'),
      width: 130,
      render: (_, command) => source(command)
    },
    {
      key: 'description',
      title: t('command_description'),
      width: 340,
      render: (_, command) => description(command)
    },
    {
      key: 'enabled',
      title: t('enable_status'),
      width: 86,
      align: 'center',
      render: (_, command) => (
        <span className="communication-commands__toggle">
          <Switch
            aria-label={t('command_toggle', { command: command.name })}
            checked={command.enabled}
            disabled={
              !canWrite || disabled || (!command.available && !command.enabled)
            }
            onChange={(enabled) =>
              canWrite && onChange({ command_id: command.command_id, enabled })
            }
          />
        </span>
      )
    }
  ];
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'communications.commands'
  });
  return (
    <DataTable<CommunicationCommandView>
      className="communication-commands"
      rowKey={(command) =>
        `${command.command_id}:${command.installation_id ?? ''}`
      }
      columns={columns}
      configuration={configuration}
      loading={loading}
      dataSource={commands}
      emptyText={loading ? null : t('no_commands')}
      page={1}
      pageSize={Math.max(commands.length, 1)}
      total={commands.length}
      onPageChange={() => {}}
    />
  );
}
