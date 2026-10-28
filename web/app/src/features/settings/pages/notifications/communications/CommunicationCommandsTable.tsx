import { useId } from 'react';
import { Flex, Grid, Switch, Table, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
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
  const screens = Grid.useBreakpoint();
  const id = useId();
  // Static translation calls keep built-in copy visible to i18n hygiene.
  const descriptions = new Map([
    ['host/new', t('command_new_description')],
    ['host/model', t('command_model_description')],
    ['host/help', t('command_help_description')]
  ]);
  const reasons = new Map([
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
  return (
    <Table<CommunicationCommandView>
      className="communication-commands"
      rowKey={(command) =>
        JSON.stringify([command.command_id, command.installation_id])
      }
      size="small"
      tableLayout="fixed"
      pagination={false}
      loading={loading}
      dataSource={commands}
      locale={{ emptyText: loading ? null : t('no_commands') }}
      columns={[
        {
          key: 'name',
          title: t('command'),
          width: screens.md ? 150 : undefined,
          render: (_, command) => (
            <Flex vertical gap={8}>
              <Typography.Text code>
                {command.usage || command.name}
              </Typography.Text>
              {!screens.md && (
                <>
                  {source(command)}
                  {description(command)}
                </>
              )}
            </Flex>
          )
        },
        {
          key: 'source_kind',
          title: t('command_source'),
          width: 130,
          responsive: ['md'],
          render: (_, command) => source(command)
        },
        {
          key: 'description',
          title: t('command_description'),
          responsive: ['md'],
          render: (_, command) => description(command)
        },
        {
          key: 'enabled',
          title: t('enable_status'),
          width: 86,
          align: 'center',
          render: (_, command) => (
            <label
              className="communication-commands__toggle"
              htmlFor={`${id}-${command.installation_id}-${command.command_id}`}
            >
              <Switch
                id={`${id}-${command.installation_id}-${command.command_id}`}
                aria-label={t('command_toggle', { command: command.name })}
                checked={command.enabled}
                disabled={disabled || (!command.available && !command.enabled)}
                onChange={(enabled) =>
                  onChange({ command_id: command.command_id, enabled })
                }
              />
            </label>
          )
        }
      ]}
    />
  );
}
