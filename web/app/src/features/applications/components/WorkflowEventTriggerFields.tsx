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
  const available =
    catalog.data?.filter(
      (entry) => entry.origin === 'custom' && entry.enabled
    ) ?? [];
  const selected = available.find(
    (entry) =>
      entry.contract_id === contract_id &&
      entry.contract_version === contract_version
  );
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
          contract_id && !selected && !catalog.isPending ? 'error' : undefined
        }
        help={
          contract_id && !selected && !catalog.isPending
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
          options={available.map((entry) => ({
            value: JSON.stringify([entry.contract_id, entry.contract_version]),
            label: `${entry.name} · ${entry.contract_version}`
          }))}
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
        rules={[
          { required: true },
          {
            validator: () =>
              selected
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
        rules={[
          { required: true },
          {
            validator: (_, value) =>
              selected?.supported_phases.includes(value)
                ? Promise.resolve()
                : Promise.reject(new Error(t('event.source_unavailable')))
          }
        ]}
      >
        <Select
          options={selected?.supported_phases.map((value) => ({
            value,
            label: {
              subscribed: t('event.subscribed'),
              published: t('event.published'),
              consumed: t('event.consumed')
            }[value]
          }))}
        />
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
              { required: field.required && field.defaultValue === undefined },
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
