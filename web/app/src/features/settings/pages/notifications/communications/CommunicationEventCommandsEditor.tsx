import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  CommunicationCommandView,
  CommunicationEventCommandBinding
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import {
  DataTable,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import { communicationEventCatalogOptions } from '../../../../notifications/api/communications';

const POINTERS = [
  '/arguments',
  '/input_id',
  '/conversation_id',
  '/instance_id'
];
const TOKEN = /^[a-z][a-z0-9_-]{0,63}$/;

export function CommunicationEventCommandsEditor({
  value,
  commands,
  disabled,
  onChange
}: {
  value: CommunicationEventCommandBinding[];
  commands: CommunicationCommandView[];
  disabled: boolean;
  onChange: (value: CommunicationEventCommandBinding[]) => void;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const [editing, setEditing] = useState<number | null | undefined>();
  const [customFields, setCustomFields] = useState<string[]>([]);
  const [newField, setNewField] = useState('');
  const [form] = Form.useForm<CommunicationEventCommandBinding>();
  const catalog = useQuery(
    communicationEventCatalogOptions(scope, editing !== undefined)
  );
  const contract_id = Form.useWatch('contract_id', form);
  const contract_version = Form.useWatch('contract_version', form);
  const targets =
    catalog.data?.filter((item) => item.enabled && item.origin === 'custom') ??
    [];
  const target = targets.find(
    (item) =>
      item.contract_id === contract_id &&
      item.contract_version === contract_version
  );
  const schema =
    target &&
    target.payload_schema !== null &&
    typeof target.payload_schema === 'object'
      ? target.payload_schema
      : undefined;
  const properties =
    schema?.properties && typeof schema.properties === 'object'
      ? (schema.properties as Record<string, unknown>)
      : {};
  const required = Array.isArray(schema?.required)
    ? (schema.required as string[])
    : [];
  const fields = [
    ...new Set([
      ...customFields,
      ...Object.keys(properties),
      ...Object.keys(Form.useWatch('input_mapping', form) ?? {}),
      ...required
    ])
  ];
  function edit(index: number | null) {
    form.resetFields();
    setCustomFields([]);
    setNewField('');
    form.setFieldsValue(
      index === null
        ? {
            command_id: '',
            name: '',
            description: '',
            enabled: true,
            contract_id: '',
            contract_version: '',
            input_mapping: {}
          }
        : value[index]
    );
    setEditing(index);
  }
  const columns: DataTableColumn<CommunicationEventCommandBinding>[] = [
    {
      key: 'name',
      title: t('command'),
      width: 180,
      mobileRole: 'title',
      render: (_, item) => <Typography.Text code>/{item.name}</Typography.Text>
    },
    {
      key: 'contract_id',
      title: t('event_commands.target'),
      width: 250,
      render: (_, item) => (
        <Flex vertical>
          <Typography.Text>
            {item.contract_id} · {item.contract_version}
          </Typography.Text>
          <Typography.Text type="secondary">{item.description}</Typography.Text>
          {commands.some(
            (command) =>
              command.command_id === item.command_id && !command.available
          ) && (
            <Typography.Text type="warning">
              {t('event_commands.target_unavailable')}
            </Typography.Text>
          )}
        </Flex>
      )
    },
    {
      key: 'enabled',
      title: t('enable_status'),
      width: 110,
      render: (_, item) => (
        <Switch
          aria-label={t('command_toggle', { command: item.name })}
          disabled={disabled}
          checked={item.enabled}
          onChange={(enabled) =>
            onChange(
              value.map((entry) =>
                entry.command_id === item.command_id
                  ? { ...entry, enabled }
                  : entry
              )
            )
          }
        />
      )
    },
    {
      key: 'actions',
      title: t('actions'),
      width: 180,
      mobileRole: 'actions',
      render: (_, item, index) => (
        <Space>
          <Button disabled={disabled} onClick={() => edit(index)}>
            {t('edit')}
          </Button>
          <Button
            danger
            disabled={disabled}
            onClick={() =>
              onChange(
                value.filter((entry) => entry.command_id !== item.command_id)
              )
            }
          >
            {t('delete')}
          </Button>
        </Space>
      )
    }
  ];
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'communications.event-commands'
  });
  return (
    <>
      <Flex justify="space-between" align="center">
        <Typography.Title level={5}>
          {t('event_commands.title')}
        </Typography.Title>
        <Button disabled={disabled} onClick={() => edit(null)}>
          {t('event_commands.add')}
        </Button>
      </Flex>
      <Typography.Paragraph type="secondary">
        {t('event_commands.help')}
      </Typography.Paragraph>
      <DataTable
        columns={columns}
        configuration={configuration}
        dataSource={value}
        rowKey="command_id"
        emptyText={t('event_commands.empty')}
        page={1}
        pageSize={Math.max(value.length, 1)}
        total={value.length}
        onPageChange={() => {}}
      />
      <Modal
        open={editing !== undefined}
        title={editing === null ? t('event_commands.add') : t('edit')}
        destroyOnHidden
        onCancel={() => setEditing(undefined)}
        okText={t('submit')}
        okButtonProps={{
          disabled: disabled || catalog.isError || catalog.isPending
        }}
        onOk={() => form.submit()}
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={(draft) => {
            const input_mapping = Object.fromEntries(
              Object.entries(draft.input_mapping ?? {}).filter(
                ([, pointer]) => !!pointer
              )
            );
            const next = { ...draft, input_mapping };
            onChange(
              editing === null
                ? [...value, next]
                : value.map((item, index) => (index === editing ? next : item))
            );
            setEditing(undefined);
          }}
        >
          {catalog.isError && (
            <Alert type="error" title={t('event_commands.catalog_failed')} />
          )}
          <Form.Item
            name="command_id"
            label={t('event_commands.identity')}
            extra={t('event_commands.identity_hint')}
            rules={[
              {
                required: true,
                pattern: /^user\/[a-z0-9_-]{1,64}$/,
                message: t('event_commands.identity_invalid')
              },
              {
                validator: (_, id) =>
                  value.some(
                    (item, index) => index !== editing && item.command_id === id
                  )
                    ? Promise.reject(new Error(t('event_commands.duplicate')))
                    : Promise.resolve()
              }
            ]}
          >
            <Input disabled={editing !== null} />
          </Form.Item>
          <Form.Item
            name="name"
            label={t('event_commands.name')}
            extra={t('event_commands.name_hint')}
            rules={[
              {
                required: true,
                pattern: TOKEN,
                message: t('event_commands.name_invalid')
              },
              {
                validator: (_, name) =>
                  ['new', 'model', 'help', 'model-user'].includes(name) ||
                  name?.startsWith('model-user-') ||
                  value.some(
                    (item, index) => index !== editing && item.name === name
                  ) ||
                  commands.some(
                    (item) => item.source_kind !== 'user' && item.name === name
                  )
                    ? Promise.reject(new Error(t('event_commands.duplicate')))
                    : Promise.resolve()
              }
            ]}
          >
            <Input addonBefore="/" />
          </Form.Item>
          <Form.Item
            name="description"
            label={t('command_description')}
            rules={[{ required: true, whitespace: true }]}
          >
            <Input.TextArea maxLength={4096} rows={2} />
          </Form.Item>
          <Form.Item
            name="enabled"
            label={t('enable_status')}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
          <Form.Item
            label={t('event_commands.target')}
            required
            validateStatus={
              contract_id && !target && !catalog.isPending ? 'error' : undefined
            }
            help={
              contract_id && !target && !catalog.isPending
                ? t('event_commands.target_unavailable')
                : undefined
            }
          >
            <Select
              aria-label={t('event_commands.target')}
              loading={catalog.isPending}
              value={
                contract_id
                  ? JSON.stringify([contract_id, contract_version])
                  : undefined
              }
              options={targets.map((item) => ({
                value: JSON.stringify([
                  item.contract_id,
                  item.contract_version
                ]),
                label: `${item.name} · ${item.contract_version}`
              }))}
              onChange={(selected) => {
                const [id, version] = JSON.parse(selected) as string[];
                form.setFieldValue('contract_id', id);
                form.setFieldValue('contract_version', version);
                form.setFieldValue('input_mapping', {});
                setCustomFields([]);
              }}
            />
          </Form.Item>
          <Form.Item
            name="contract_id"
            hidden
            rules={[
              {
                validator: () =>
                  target
                    ? Promise.resolve()
                    : Promise.reject(
                        new Error(t('event_commands.target_unavailable'))
                      )
              }
            ]}
          >
            <Input />
          </Form.Item>
          <Form.Item name="contract_version" hidden>
            <Input />
          </Form.Item>
          {fields.map((field) => (
            <Form.Item
              key={field}
              name={['input_mapping', field]}
              label={t('event_commands.mapping', { field })}
              rules={[
                {
                  required: required.includes(field),
                  message: t('event_commands.mapping_required')
                },
                {
                  validator: (_, pointer) =>
                    !pointer || POINTERS.includes(pointer)
                      ? Promise.resolve()
                      : Promise.reject(
                          new Error(t('event_commands.mapping_invalid'))
                        )
                }
              ]}
            >
              <Select
                allowClear
                options={POINTERS.map((pointer) => ({
                  value: pointer,
                  label: pointer
                }))}
              />
            </Form.Item>
          ))}
          <Space.Compact block>
            <Input
              aria-label={t('event_commands.payload_field')}
              value={newField}
              onChange={(event) => setNewField(event.target.value)}
            />
            <Button
              disabled={!newField.trim() || fields.includes(newField.trim())}
              onClick={() => {
                setCustomFields([...customFields, newField.trim()]);
                setNewField('');
              }}
            >
              {t('event_commands.add_mapping')}
            </Button>
          </Space.Compact>
        </Form>
      </Modal>
    </>
  );
}
