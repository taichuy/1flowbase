import { useState } from 'react';
import { Button, Flex, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  CommunicationCommandConfiguration,
  CommunicationCommandView,
  CommunicationEventCommandBinding
} from '@1flowbase/api-client';
import { CommunicationCommandEditor } from './CommunicationCommandEditor';
import { CommunicationCommandsTable } from './CommunicationCommandsTable';

export function CommunicationCommandsEditor({
  value,
  commands,
  loading,
  disabled,
  onChange
}: {
  value: CommunicationCommandConfiguration[];
  commands: CommunicationCommandView[];
  loading: boolean;
  disabled: boolean;
  onChange: (value: CommunicationCommandConfiguration[]) => void;
}) {
  const { t } = useTranslation('notifications');
  const [editing, setEditing] = useState<
    CommunicationEventCommandBinding | null | undefined
  >();
  const eventCommands = value.filter((command) => command.kind === 'event');
  return (
    <>
      <Flex justify="space-between" align="center">
        <Typography.Title level={5}>{t('commands')}</Typography.Title>
        <Button disabled={disabled} onClick={() => setEditing(null)}>
          {t('event_commands.add')}
        </Button>
      </Flex>
      <CommunicationCommandsTable
        value={value}
        commands={commands}
        loading={loading}
        disabled={disabled}
        onChange={({ command_id, enabled }) =>
          onChange(
            value.map((command) =>
              command.command_id === command_id
                ? { ...command, enabled }
                : command
            )
          )
        }
        onEdit={(command_id) =>
          setEditing(
            eventCommands.find((command) => command.command_id === command_id)
          )
        }
        onDelete={(command_id) =>
          onChange(value.filter((command) => command.command_id !== command_id))
        }
      />
      <CommunicationCommandEditor
        value={eventCommands}
        commands={commands}
        editing={editing}
        disabled={disabled}
        onClose={() => setEditing(undefined)}
        onSave={(command) => {
          const next: CommunicationCommandConfiguration = {
            kind: 'event',
            ...command
          };
          onChange(
            editing === null
              ? [...value, next]
              : value.map((item) =>
                  item.command_id === editing?.command_id ? next : item
                )
          );
          setEditing(undefined);
        }}
      />
    </>
  );
}
