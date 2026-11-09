import {
  eventCatalogIdentity,
  type EventCatalogEntry,
  type EventSourceSelection
} from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  AutoComplete,
  Form,
  Input,
  Select,
  type FormInstance
} from 'antd';
import { eventSchemaPointers } from './workflow-event-schema';
import { useTranslation } from 'react-i18next';
import {
  fetchWorkflowEventCatalog,
  workflowEventCatalogQueryKey
} from '../api/applications';

export function WorkflowEventTriggerFields({
  form,
  fields
}: {
  form: FormInstance;
  fields: Array<{
    key: string;
    label: string;
    valueType: string;
    required: boolean;
    defaultValue?: unknown;
  }>;
}) {
  const { t } = useTranslation('applications');
  const catalog = useQuery({
    queryKey: workflowEventCatalogQueryKey,
    queryFn: fetchWorkflowEventCatalog
  });
  const contract_id = Form.useWatch(['event', 'contract_id'], form);
  const trigger_phase = Form.useWatch(['event', 'trigger_phase'], form);
  const source = Form.useWatch(['event', 'source'], form) as
    | EventSourceSelection
    | undefined;
  const identity = (entry: Pick<Parameters<typeof eventCatalogIdentity>[0], 'contract_id' | 'source'>) =>
    eventCatalogIdentity({
      contract_id: entry.contract_id,
      source: entry.source
    });
  const currentIdentity = identity({
    contract_id,
    source: source ? { selection: source } : undefined
  });
  const eligible = (entry: EventCatalogEntry) =>
    (entry.origin === 'custom'
      ? entry.enabled !== false
      : Boolean(entry.source)) && entry.supported_phases.length > 0;
  const scopeLabels = {
    system: t('event.system_scope'),
    unbound: t('event.unbound_scope')
  };
  const sourceLabel = (entry: EventCatalogEntry) =>
    `${entry.group_path?.length ? `${entry.group_path.join(' / ')} / ` : ''}${entry.name}${
      entry.source
        ? ` · ${entry.source.selection.scope.kind === 'workspace' ? entry.source.selection.scope.workspace_id : scopeLabels[entry.source.selection.scope.kind]} · ${entry.source.selection.source_id}`
        : ''
    }`;
  const available = catalog.data?.filter(eligible) ?? [];
  const selected = catalog.data?.find(
    (entry) => identity(entry) === currentIdentity
  );
  const sourceAvailable = selected ? eligible(selected) : false;
  const sourceOptions = available.map((entry) => ({
    value: identity(entry),
    label: sourceLabel(entry),
    disabled: false
  }));
  if (contract_id && !sourceAvailable) {
    sourceOptions.push({
      value: currentIdentity,
      label: `${selected ? sourceLabel(selected) : contract_id} · ${t('event.source_unavailable')}`,
      disabled: true
    });
  }
  const pointerOptions = eventSchemaPointers(selected?.payload_schema).map(
    (value) => ({ value })
  );
  const phaseLabels = {
    subscribed: t('event.subscribed'),
    published: t('event.published'),
    consumed: t('event.consumed')
  };
  const phaseOptions = (selected?.supported_phases ?? []).map((value) => ({
    value,
    label: phaseLabels[value],
    disabled: !sourceAvailable
  }));
  if (
    trigger_phase &&
    !phaseOptions.some(({ value }) => value === trigger_phase)
  ) {
    phaseOptions.push({
      value: trigger_phase,
      label: phaseLabels[trigger_phase as keyof typeof phaseLabels],
      disabled: true
    });
  }
  return (
    <>
      <Alert type="info" showIcon title={t('event.listening_hint')} />
      {trigger_phase === 'consumed' && (
        <Alert type="info" showIcon title={t('event.lifecycle_hint')} />
      )}
      {catalog.isError && (
        <Alert type="error" title={t('auto.trigger_load_failed')} />
      )}
      <Form.Item
        label={t('event.source')}
        required
        validateStatus={
          contract_id && !sourceAvailable && !catalog.isPending
            ? 'error'
            : undefined
        }
        help={
          contract_id && !sourceAvailable && !catalog.isPending
            ? t('event.source_unavailable')
            : undefined
        }
      >
        <Select
          aria-label={t('event.source')}
          loading={catalog.isPending}
          value={contract_id ? currentIdentity : undefined}
          options={sourceOptions}
          onChange={(value) => {
            const entry = available.find((entry) => identity(entry) === value);
            if (!entry) return;
            form.setFieldValue(['event', 'contract_id'], entry.contract_id);
            form.setFieldValue(['event', 'source'], entry.source?.selection);
            form.setFieldValue(
              ['event', 'trigger_phase'],
              entry.supported_phases.includes('published')
                ? 'published'
                : undefined
            );
            form.setFieldValue(['event', 'input_mapping'], {});
          }}
        />
      </Form.Item>
      <Form.Item
        name={['event', 'contract_id']}
        hidden
        rules={[
          { required: true },
          {
            validator: () =>
              sourceAvailable
                ? Promise.resolve()
                : Promise.reject(new Error(t('event.source_unavailable')))
          }
        ]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        name={['event', 'source']}
        hidden
        getValueProps={() => ({ value: '' })}
      >
        <Input />
      </Form.Item>
      <Form.Item
        label={t('event.phase')}
        name={['event', 'trigger_phase']}
        dependencies={[
          ['event', 'contract_id'],
          ['event', 'source']
        ]}
        rules={[
          { required: true },
          {
            validator: (_, value) =>
              sourceAvailable && selected?.supported_phases.includes(value)
                ? Promise.resolve()
                : Promise.reject(new Error(t('event.source_unavailable')))
          }
        ]}
      >
        <Select options={phaseOptions} />
      </Form.Item>
      {/* Register the whole mapping even when no Start input controls exist.
          Disabling a stored source must retain mappings for historical inputs. */}
      <Form.Item
        name={['event', 'input_mapping']}
        hidden
        getValueProps={() => ({ value: '' })}
      >
        <Input />
      </Form.Item>
      {fields
        .filter((field) =>
          ['string', 'number', 'boolean'].includes(field.valueType)
        )
        .map((field) => (
          <Form.Item
            key={field.key}
            label={`${field.label} · ${field.valueType}`}
            name={['event', 'input_mapping', field.key]}
            rules={[
              {
                required: field.required && field.defaultValue === undefined
              },
              {
                pattern: /^(?:\/(?:[^~]|~[01])*)*$/,
                message: t('event.pointer_invalid')
              }
            ]}
            extra={t('event.pointer_hint')}
          >
            <AutoComplete
              options={pointerOptions}
              filterOption={(input, option) =>
                !!option?.value.toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>
        ))}
    </>
  );
}
