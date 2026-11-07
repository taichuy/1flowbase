import {
  getApiEventOperation,
  listApiEventOperations,
  type ApiEventUnavailableReason,
  type CreateEventDefinitionInput,
  type EventCatalogEntry,
  type InterfaceEventPhase,
  type InterfaceEventTerminal
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
import { useQuery } from '@tanstack/react-query';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { useTranslation } from 'react-i18next';
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';

export function EventDefinitionDrawer({
  entry,
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
  const can = useConsoleOperationAccess();
  const operation_id = Form.useWatch(
    ['interface_event', 'operation_id'],
    form
  ) as string | undefined;
  const phase = Form.useWatch(['interface_event', 'phase'], form) as
    | InterfaceEventPhase
    | undefined;
  const enabled = Form.useWatch('enabled', form) as boolean | undefined;
  const legacy =
    entry?.interface_event && 'interface_id' in entry.interface_event
      ? entry.interface_event
      : undefined;
  const operations = useQuery({
    queryKey: ['events', 'api-operations'],
    queryFn: () => listApiEventOperations(),
    enabled: editable && !legacy && can('events.api_operations.list')
  });
  const details = useQuery({
    queryKey: ['events', 'api-operations', operation_id],
    queryFn: () => getApiEventOperation(operation_id!),
    enabled:
      editable && !legacy && !!operation_id && can('events.api_operations.get')
  });
  const selected =
    details.data?.operation_id === operation_id ? details.data : undefined;
  const availability = phase && selected ? selected[phase] : undefined;
  const selectedSchema = legacy
    ? entry?.payload_schema
    : phase === 'before'
      ? selected?.before_schema
      : phase === 'completion'
        ? selected?.completion_schema
        : undefined;
  const hasSchema = selectedSchema !== undefined && selectedSchema !== null;
  const binding = Form.useWatch('interface_event', form);
  const unchangedDisabled =
    enabled === false &&
    entry?.interface_event !== undefined &&
    JSON.stringify(binding) === JSON.stringify(entry.interface_event);
  const canSave = legacy
    ? !!entry
    : (availability?.status === 'available' && hasSchema) || unchangedDisabled;
  // Static references keep all backend reason translations visible to i18n hygiene.
  const reasonLabels: Record<ApiEventUnavailableReason, string> = {
    missing_openapi_contract: t('api_reason_missing_openapi_contract'),
    missing_binding: t('api_reason_missing_binding'),
    missing_typed_codec: t('api_reason_missing_typed_codec'),
    no_workspace_principal: t('api_reason_no_workspace_principal'),
    permission_denied: t('api_reason_permission_denied'),
    target_unavailable: t('api_reason_target_unavailable'),
    binary_request: t('api_reason_binary_request'),
    binary_response: t('api_reason_binary_response'),
    missing_streaming_terminal: t('api_reason_missing_streaming_terminal'),
    credential_boundary: t('api_reason_credential_boundary'),
    ambiguous_operation: t('api_reason_ambiguous_operation')
  };
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
            if (canSave)
              onSave({
                name,
                description: description ?? '',
                enabled,
                interface_event: legacy ?? {
                  operation_id:
                    'operation_id' in interface_event
                      ? interface_event.operation_id
                      : '',
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
          {legacy ? (
            <Alert
              type="info"
              showIcon
              title={t('legacy_rule')}
              description={legacy.interface_id}
            />
          ) : (
            <>
              {(operations.isError || details.isError) && (
                <Alert
                  type="error"
                  showIcon
                  title={t('api_catalog_error')}
                  action={
                    <Button
                      onClick={() => {
                        void operations.refetch();
                        if (operation_id) void details.refetch();
                      }}
                    >
                      {t('refresh')}
                    </Button>
                  }
                />
              )}
              <Form.Item
                name={['interface_event', 'operation_id']}
                label={t('interface_id')}
                rules={[{ required: true }]}
              >
                <Select
                  showSearch
                  optionFilterProp="label"
                  loading={operations.isFetching}
                  options={(operations.data ?? []).map((item) => ({
                    value: item.operation_id,
                    label: `${item.method.toUpperCase()} ${item.path} · ${item.name}`
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
                  disabled={!selected || saving}
                  loading={details.isFetching}
                  options={(['before', 'completion'] as const).map((value) => ({
                    value,
                    label: phaseLabels[value]
                  }))}
                  onChange={() =>
                    form.setFieldValue(['interface_event', 'results'], [])
                  }
                />
              </Form.Item>
              {availability?.status === 'unavailable' && (
                <Alert
                  type="warning"
                  showIcon
                  title={reasonLabels[availability.reason]}
                />
              )}
            </>
          )}
          {!legacy && phase === 'completion' && (
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
          {hasSchema ? (
            <JsonPreviewBlock
              title={t('payload_schema')}
              value={selectedSchema}
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
              disabled={!canSave || (legacy ? false : details.isFetching)}
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
