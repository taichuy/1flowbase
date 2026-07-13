import { useEffect, useMemo } from 'react';

import {
  Alert,
  Button,
  Drawer,
  Form,
  Input,
  InputNumber,
  Select,
  Switch
} from 'antd';

import type {
  CreateSettingsDataSourceInput,
  SettingsDataSourceCatalogEntry
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

interface DataSourceFormValues {
  installation_id: string;
  display_name: string;
  values: Record<string, unknown>;
}

function schemaFieldControl(field: SettingsDataSourceCatalogEntry['config_schema'][number]) {
  if (field.options.length > 0 || field.control === 'select') {
    return (
      <Select
        options={field.options.map((option) => ({
          label: option.label,
          value: option.value as string | number,
          disabled: option.disabled ?? false
        }))}
      />
    );
  }
  if (field.field_type === 'boolean' || field.control === 'switch') {
    return <Switch />;
  }
  if (field.field_type === 'number' || field.field_type === 'integer') {
    return <InputNumber style={{ width: '100%' }} />;
  }
  if (field.send_mode === 'secret_ref' || field.control === 'password') {
    return <Input.Password placeholder={field.placeholder ?? undefined} />;
  }
  return <Input placeholder={field.placeholder ?? undefined} />;
}

export function DataSourceCreateDrawer({
  open,
  catalog,
  saving,
  errorMessage,
  onClose,
  onCreate
}: {
  open: boolean;
  catalog: SettingsDataSourceCatalogEntry[];
  saving: boolean;
  errorMessage: string | null;
  onClose: () => void;
  onCreate: (input: CreateSettingsDataSourceInput) => Promise<void>;
}) {
  const [form] = Form.useForm<DataSourceFormValues>();
  const installationId = Form.useWatch('installation_id', form);
  const selectedExtension = useMemo(
    () => catalog.find((entry) => entry.installation_id === installationId),
    [catalog, installationId]
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    form.resetFields();
    if (catalog.length === 1) {
      form.setFieldValue('installation_id', catalog[0].installation_id);
    }
  }, [catalog, form, open]);

  const submit = async () => {
    const values = await form.validateFields();
    const extension = catalog.find(
      (entry) => entry.installation_id === values.installation_id
    );
    if (!extension) {
      return;
    }

    const configJson: Record<string, unknown> = {};
    const secretJson: Record<string, unknown> = {};
    for (const field of extension.config_schema) {
      const value = values.values?.[field.key];
      if (value === undefined) {
        continue;
      }
      if (field.send_mode === 'secret_ref') {
        secretJson[field.key] = value;
      } else {
        configJson[field.key] = value;
      }
    }

    try {
      await onCreate({
        installation_id: extension.installation_id,
        source_code: extension.source_code,
        display_name: values.display_name,
        config_json: configJson,
        secret_json: secretJson
      });
    } catch {
      // The mutation error is rendered in this Drawer so the form stays retryable.
    }
  };

  return (
    <Drawer
      title={i18nText('settings', 'auto.add_data_source')}
      open={open}
      width={520}
      onClose={onClose}
      extra={
        <Button
          type="primary"
          aria-label={i18nText('settings', 'auto.create')}
          loading={saving}
          onClick={submit}
        >
          {i18nText('settings', 'auto.create')}
        </Button>
      }
    >
      <Form form={form} layout="vertical">
        {errorMessage ? (
          <Alert
            type="error"
            showIcon
            message={errorMessage}
            style={{ marginBottom: 16 }}
          />
        ) : null}
        <Form.Item
          name="installation_id"
          label={i18nText('settings', 'auto.data_source_extension')}
          rules={[{ required: true }]}
        >
          <Select
            options={catalog.map((entry) => ({
              label: entry.display_name,
              value: entry.installation_id
            }))}
          />
        </Form.Item>
        <Form.Item
          name="display_name"
          label={i18nText('settings', 'auto.data_source_name')}
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        {selectedExtension?.config_schema.map((field) => (
          <Form.Item
            key={field.key}
            name={['values', field.key]}
            label={field.label}
            help={field.description}
            initialValue={field.default_value}
            valuePropName={
              field.field_type === 'boolean' || field.control === 'switch'
                ? 'checked'
                : 'value'
            }
            rules={[{ required: field.required ?? false }]}
          >
            {schemaFieldControl(field)}
          </Form.Item>
        ))}
      </Form>
    </Drawer>
  );
}
