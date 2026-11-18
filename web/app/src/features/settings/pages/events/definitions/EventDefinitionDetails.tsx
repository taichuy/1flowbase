import type {
  EventCatalogEntry,
  InterfaceEventTerminal
} from '@1flowbase/api-client';
import { Alert, Descriptions, Space, Switch } from 'antd';
import { useTranslation } from 'react-i18next';
import { JsonPreviewBlock } from '../../../../../shared/ui/json-preview/JsonPreviewBlock';

export function EventDefinitionDetails({
  entry
}: {
  entry: EventCatalogEntry;
}) {
  const { t } = useTranslation('settingsEvents');
  const phaseLabels = { before: t('before'), completion: t('completion') };
  const resultLabels: Record<InterfaceEventTerminal, string> = {
    completed: t('completed'),
    failed: t('failed'),
    rejected: t('rejected'),
    cancelled: t('cancelled')
  };
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {entry.origin !== 'custom' && (
        <Alert type="info" showIcon title={t('read_only')} />
      )}
      <Descriptions
        column={1}
        items={[
          ...(entry.event_kind
            ? [
                {
                  key: 'event_kind',
                  label: t('event_kind'),
                  children:
                    entry.event_kind === 'communication_command'
                      ? t('communication_command_event')
                      : t('interface_event')
                }
              ]
            : []),
          {
            key: 'contract_id',
            label: t('contract_id'),
            children: entry.contract_id
          },
          {
            key: 'group_path',
            label: t('group_path'),
            children: entry.group_path?.join(' / ') || '—'
          },
          {
            key: 'owner_id',
            label: t('owner_id'),
            children: entry.owner_id ?? '—'
          },
          {
            key: 'scope',
            label: t('scope'),
            children:
              entry.scope.kind === 'workspace'
                ? `${t('workspace_id')} · ${entry.scope.workspace_id}`
                : entry.scope.kind === 'system'
                  ? t('core')
                  : t(entry.scope.kind)
          },
          {
            key: 'enabled',
            label: t('enabled'),
            children: (
              <Switch
                aria-label={t('enabled')}
                checked={entry.origin !== 'custom' || entry.enabled === true}
                disabled
              />
            )
          },
          ...(entry.interface_event
            ? [
                {
                  key: 'interface_id',
                  label: t('interface_id'),
                  children:
                    'operation_id' in entry.interface_event
                      ? entry.interface_event.operation_id
                      : entry.interface_event.interface_id
                },
                {
                  key: 'interface_phase',
                  label: t('interface_phase'),
                  children: phaseLabels[entry.interface_event.phase]
                },
                ...(entry.interface_event.phase === 'completion'
                  ? [
                      {
                        key: 'results',
                        label: t('results'),
                        children: entry.interface_event.results?.length
                          ? entry.interface_event.results
                              .map((result) => resultLabels[result])
                              .join(' / ')
                          : t('all_results')
                      }
                    ]
                  : [])
              ]
            : []),
          ...(entry.source
            ? [
                {
                  key: 'source_id',
                  label: t('source_id'),
                  children: entry.source.selection.source_id
                },
                {
                  key: 'point_id',
                  label: t('point_id'),
                  children: entry.source.point_id
                },
                ...Object.entries(entry.source.selection.filters ?? {}).map(
                  ([key, value]) => ({
                    key: `filter:${key}`,
                    label: key,
                    children: value
                  })
                )
              ]
            : []),
          {
            key: 'description',
            label: t('description'),
            children: entry.description || '—'
          }
        ]}
      />
      <JsonPreviewBlock
        title={t('payload_schema')}
        value={entry.payload_schema}
        maxAutoHeight={400}
      />
    </Space>
  );
}
