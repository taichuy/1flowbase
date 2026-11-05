import type {
  CreateEventDefinitionInput,
  EventCatalogEntry
} from '@1flowbase/api-client';
import {
  Alert,
  Button,
  Descriptions,
  Drawer,
  Form,
  Input,
  Space,
  Switch,
  Typography
} from 'antd';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { JsonSchemaInlineEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel';

export function EventDefinitionDrawer({
  entry,
  saving,
  editable,
  error,
  onClose,
  onSave
}: {
  entry: EventCatalogEntry | null;
  saving: boolean;
  editable: boolean;
  error: boolean;
  onClose: () => void;
  onSave: (values: CreateEventDefinitionInput) => void;
}) {
  const { t } = useTranslation('settingsEvents');
  const [schema, setSchema] = useState<
    Record<string, unknown> | boolean | null
  >(entry ? entry.payload_schema : { type: 'object', properties: {} });
  const [valid, setValid] = useState(true);
  return (
    <Drawer
      open
      destroyOnHidden
      size="min(760px, 100vw)"
      title={entry ? entry.name : t('create')}
      onClose={onClose}
    >
      {error && <Alert type="error" showIcon title={t('save_error')} />}
      {editable && schema !== null ? (
        <Form<CreateEventDefinitionInput>
          layout="vertical"
          disabled={saving}
          initialValues={
            entry ?? { contract_version: '1', enabled: true, description: '' }
          }
          onFinish={(values) => {
            if (valid && schema !== null)
              onSave({ ...values, payload_schema: schema });
          }}
        >
          <Form.Item
            name="name"
            label={t('name')}
            rules={[{ required: true, whitespace: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="contract_id"
            label={t('contract_id')}
            rules={[
              { required: true },
              {
                pattern: /^custom\.[a-z0-9_-]+(?:\.[a-z0-9_-]+)*$/,
                message: t('contract_help')
              }
            ]}
          >
            <Input disabled={!!entry} />
          </Form.Item>
          <Form.Item
            name="contract_version"
            label={t('contract_version')}
            rules={[{ required: true, whitespace: true }]}
          >
            <Input disabled={!!entry} />
          </Form.Item>
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
          <Form.Item label={t('payload_schema')} required>
            {typeof schema === 'boolean' ? (
              <Alert
                type="info"
                title={schema ? t('accepts_all') : t('rejects_all')}
              />
            ) : (
              <JsonSchemaInlineEditor
                schema={schema}
                resetKey={entry?.id ?? 'create'}
                structureMode="fields"
                onChange={setSchema}
                onValidityChange={setValid}
              />
            )}
          </Form.Item>
          <Space>
            <Button
              type="primary"
              htmlType="submit"
              loading={saving}
              disabled={!valid}
            >
              {t('save')}
            </Button>
            <Button onClick={onClose}>{t('close')}</Button>
          </Space>
        </Form>
      ) : (
        entry && (
          <Space orientation="vertical" size="middle">
            <Alert type="info" showIcon title={t('read_only')} />
            <Descriptions
              column={1}
              items={[
                {
                  key: 'contract_id',
                  label: t('contract_id'),
                  children: entry.contract_id
                },
                {
                  key: 'contract_version',
                  label: t('contract_version'),
                  children: entry.contract_version
                },
                {
                  key: 'owner_id',
                  label: t('owner_id'),
                  children: entry.owner_id
                },
                {
                  key: 'description',
                  label: t('description'),
                  children: entry.description || '—'
                }
              ]}
            />
            <Typography.Text>{t('payload_schema')}</Typography.Text>
            {entry.payload_schema === null && (
              <Alert type="info" title={t('schema_undeclared')} />
            )}
            {typeof entry.payload_schema === 'boolean' && (
              <Alert
                type="info"
                title={
                  entry.payload_schema ? t('accepts_all') : t('rejects_all')
                }
              />
            )}
            {entry.payload_schema !== null &&
              typeof entry.payload_schema !== 'boolean' && (
                <Descriptions
                  column={1}
                  items={Object.entries(
                    (entry.payload_schema.properties ?? {}) as Record<
                      string,
                      Record<string, unknown>
                    >
                  ).map(([name, field]) => ({
                    key: name,
                    label: name,
                    children:
                      typeof field === 'object' &&
                      field !== null &&
                      typeof field.type === 'string'
                        ? field.type
                        : t('structured_field')
                  }))}
                />
              )}
          </Space>
        )
      )}
    </Drawer>
  );
}
