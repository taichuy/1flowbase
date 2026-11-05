import { useQuery } from '@tanstack/react-query';
import { Alert, Form, Input, Select, Switch, type FormInstance } from 'antd';
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
  const contract_version = Form.useWatch(['event', 'contract_version'], form);
  const enabled = Form.useWatch(['event', 'enabled'], form);
  const trigger_phase = Form.useWatch(['event', 'trigger_phase'], form);
  const available =
    catalog.data?.filter(
      (entry) => entry.origin === 'custom' && entry.enabled
    ) ?? [];
  const selected = catalog.data?.find(
    (entry) =>
      entry.contract_id === contract_id &&
      entry.contract_version === contract_version
  );
  const sourceAvailable = selected?.origin === 'custom' && selected.enabled;
  const sourceOptions = available.map((entry) => ({
    value: JSON.stringify([entry.contract_id, entry.contract_version]),
    label: `${entry.name} · ${entry.contract_version}`,
    disabled: false
  }));
  if (contract_id && !sourceAvailable) {
    sourceOptions.push({
      value: JSON.stringify([contract_id, contract_version]),
      label: `${selected?.name ?? contract_id} · ${contract_version} · ${t('event.source_unavailable')}`,
      disabled: true
    });
  }
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
      <Alert type="info" showIcon title={t('event.lifecycle_hint')} />
      {catalog.isError && (
        <Alert type="error" title={t('auto.trigger_load_failed')} />
      )}
      <Form.Item
        label={t('event.subscription_enabled')}
        name={['event', 'enabled']}
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>
      <Form.Item
        label={t('event.source')}
        required
        validateStatus={
          enabled && contract_id && !sourceAvailable && !catalog.isPending
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
          value={
            contract_id
              ? JSON.stringify([contract_id, contract_version])
              : undefined
          }
          options={sourceOptions}
          onChange={(value) => {
            const [contract_id, contract_version] = JSON.parse(
              value
            ) as string[];
            form.setFieldValue(['event', 'contract_id'], contract_id);
            form.setFieldValue(['event', 'contract_version'], contract_version);
            form.setFieldValue(['event', 'trigger_phase'], undefined);
            form.setFieldValue(['event', 'input_mapping'], {});
          }}
        />
      </Form.Item>
      <Form.Item
        name={['event', 'contract_id']}
        hidden
        dependencies={[['event', 'enabled']]}
        rules={[
          { required: true },
          {
            validator: () =>
              !form.getFieldValue(['event', 'enabled']) || sourceAvailable
                ? Promise.resolve()
                : Promise.reject(new Error(t('event.source_unavailable')))
          }
        ]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        name={['event', 'contract_version']}
        hidden
        rules={[{ required: true }]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        label={t('event.phase')}
        name={['event', 'trigger_phase']}
        dependencies={[
          ['event', 'enabled'],
          ['event', 'contract_id'],
          ['event', 'contract_version']
        ]}
        rules={[
          { required: true },
          {
            validator: (_, value) =>
              !form.getFieldValue(['event', 'enabled']) ||
              (sourceAvailable && selected?.supported_phases.includes(value))
                ? Promise.resolve()
                : Promise.reject(new Error(t('event.source_unavailable')))
          }
        ]}
      >
        <Select options={phaseOptions} />
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
                required:
                  enabled && field.required && field.defaultValue === undefined
              },
              {
                pattern: /^(?:\/(?:[^~]|~[01])*)*$/,
                message: t('event.pointer_invalid')
              }
            ]}
            extra={t('event.pointer_hint')}
          >
            <Input />
          </Form.Item>
        ))}
    </>
  );
}
