import type {
  CreateEventDefinitionInput,
  EventCatalogEntry,
  InterfaceEventPhase,
  InterfaceEventTerminal
} from '@1flowbase/api-client';
import {
  Alert,
  Button,
  Descriptions,
  Drawer,
  Form,
  Input,
  Select,
  Space,
  Switch
} from 'antd';
import { useTranslation } from 'react-i18next';
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';

export function EventDefinitionDrawer({
  entry,
  catalog,
  saving,
  editable,
  error,
  onClose,
  onSave
}: {
  entry: EventCatalogEntry | null;
  catalog: EventCatalogEntry[];
  saving: boolean;
  editable: boolean;
  error: boolean;
  onClose: () => void;
  onSave: (values: CreateEventDefinitionInput) => void;
}) {
  const { t } = useTranslation('settingsEvents');
  const [form] = Form.useForm<CreateEventDefinitionInput>();
  const interface_id = Form.useWatch(['interface_event', 'interface_id'], form);
  const phase = Form.useWatch(['interface_event', 'phase'], form) as
    | InterfaceEventPhase
    | undefined;
  const declarations = catalog.filter(
    (item) =>
      item.origin !== 'custom' &&
      item.interface_event &&
      item.payload_schema !== null &&
      item.supported_phases.length > 0
  );
  const interfaces = [
    ...new Map(
      declarations.map((item) => [item.interface_event!.interface_id, item])
    ).values()
  ];
  const phases = declarations.filter(
    (item) => item.interface_event?.interface_id === interface_id
  );
  const selected = phases.find((item) => item.interface_event?.phase === phase);
  const phaseLabels = { before: t('before'), completion: t('completion') };
  const resultLabels: Record<InterfaceEventTerminal, string> = {
    completed: t('completed'),
    failed: t('failed'),
    rejected: t('rejected'),
    cancelled: t('cancelled')
  };
  return (
    <Drawer
      open
      destroyOnHidden
      size="min(760px, 100vw)"
      title={entry ? entry.name : t('create')}
      onClose={onClose}
    >
      {error && <Alert type="error" showIcon title={t('save_error')} />}
      {editable ? (
        <Form<CreateEventDefinitionInput>
          form={form}
          layout="vertical"
          disabled={saving}
          initialValues={
            entry
              ? {
                  name: entry.name,
                  description: entry.description,
                  enabled: entry.enabled,
                  interface_event: entry.interface_event
                }
              : { enabled: true, description: '' }
          }
          onFinish={({ name, description, enabled, interface_event }) => {
            if (selected)
              onSave({
                name,
                description: description ?? '',
                enabled,
                interface_event: {
                  interface_id: interface_event.interface_id,
                  phase: interface_event.phase,
                  ...(interface_event.phase === 'completion'
                    ? { results: interface_event.results ?? [] }
                    : {})
                }
              });
          }}
        >
          <Alert type="info" showIcon title={t('interface_help')} />
          <Form.Item
            name="name"
            label={t('name')}
            rules={[{ required: true, whitespace: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name={['interface_event', 'interface_id']}
            label={t('interface_id')}
            rules={[{ required: true }]}
          >
            <Select
              showSearch
              optionFilterProp="label"
              options={interfaces.map((item) => ({
                value: item.interface_event!.interface_id,
                label: `${item.group_path?.join(' / ') || item.name} · ${item.interface_event!.interface_id}`
              }))}
              onChange={() => {
                form.setFieldValue(['interface_event', 'phase'], undefined);
                form.setFieldValue(['interface_event', 'results'], []);
              }}
              notFoundContent={t('no_interfaces')}
            />
          </Form.Item>
          <Form.Item
            name={['interface_event', 'phase']}
            label={t('interface_phase')}
            rules={[{ required: true }]}
          >
            <Select
              disabled={!interface_id || saving}
              options={[
                ...new Set(phases.map((item) => item.interface_event!.phase))
              ].map((value) => ({ value, label: phaseLabels[value] }))}
              onChange={() =>
                form.setFieldValue(['interface_event', 'results'], [])
              }
            />
          </Form.Item>
          {phase === 'completion' && (
            <Form.Item
              name={['interface_event', 'results']}
              label={t('results')}
              extra={t('results_help')}
            >
              <Select
                mode="multiple"
                allowClear
                options={(
                  Object.keys(resultLabels) as InterfaceEventTerminal[]
                ).map((value) => ({ value, label: resultLabels[value] }))}
              />
            </Form.Item>
          )}
          <Form.Item name="description" label={t('description')}>
            <Input.TextArea autoSize={{ minRows: 2, maxRows: 6 }} />
          </Form.Item>
          <Form.Item
            name="enabled"
            label={t('enabled')}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
          {selected ? (
            <JsonPreviewBlock
              title={t('payload_schema')}
              value={selected.payload_schema}
              maxAutoHeight={400}
            />
          ) : (
            <Alert type="info" title={t('select_interface_phase')} />
          )}
          <Space>
            <Button
              type="primary"
              htmlType="submit"
              loading={saving}
              disabled={!selected}
            >
              {t('save')}
            </Button>
            <Button onClick={onClose}>{t('close')}</Button>
          </Space>
        </Form>
      ) : (
        entry && (
          <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
            {entry.origin !== 'custom' && (
              <Alert type="info" showIcon title={t('read_only')} />
            )}
            <Descriptions
              column={1}
              items={[
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
                      ? `${t('workspace_scope')} · ${entry.scope.workspace_id}`
                      : t(entry.scope.kind)
                },
                {
                  key: 'enabled',
                  label: t('enabled'),
                  children: (
                    <Switch
                      aria-label={t('enabled')}
                      checked={
                        entry.origin !== 'custom' || entry.enabled === true
                      }
                      disabled
                    />
                  )
                },
                ...(entry.interface_event
                  ? [
                      {
                        key: 'interface_id',
                        label: t('interface_id'),
                        children: entry.interface_event.interface_id
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
                      ...Object.entries(
                        entry.source.selection.filters ?? {}
                      ).map(([key, value]) => ({
                        key: `filter:${key}`,
                        label: key,
                        children: value
                      }))
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
        )
      )}
    </Drawer>
  );
}
