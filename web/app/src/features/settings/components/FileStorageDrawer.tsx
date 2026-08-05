import { useEffect, useState } from 'react';
import {
  Button,
  Drawer,
  Form,
  Input,
  InputNumber,
  App,
  Select,
  Switch
} from 'antd';
import {
  createSettingsFileStorage,
  updateSettingsFileStorage,
  type SettingsFileStorage,
  type CreateSettingsFileStorageInput,
  type UpdateSettingsFileStorageInput
} from '../api/file-management';
import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';

type DrawerMode = 'create' | 'view' | 'edit';

interface FileStorageDrawerProps {
  open: boolean;
  mode: DrawerMode;
  record: SettingsFileStorage | null;
  onClose: () => void;
  onSuccess: () => void;
}

type StorageFormScalar = string | number | boolean | undefined;
type StorageFormObject = Record<string, StorageFormScalar>;

interface StorageFormValues {
  code: string;
  title: string;
  driver_type: string;
  enabled: boolean;
  is_default: boolean;
  config_json: StorageFormObject;
  rule_json: StorageFormObject;
}

const DRIVER_TYPE_OPTIONS = [
  { label: i18nText('settings', 'auto.local_file_system'), value: 'local' },
  { label: i18nText('settings', 'auto.aws_s_three_compatible'), value: 's3' },
  { label: i18nText('settings', 'auto.alibaba_cloud_oss'), value: 'oss' },
  { label: i18nText('settings', 'auto.tencent_cloud_cos'), value: 'cos' },
  {
    label: i18nText('settings', 'auto.rustfs_s_three_compatible'),
    value: 'rustfs'
  }
];

const DRIVER_FIELDS: Record<
  string,
  { key: string; label: string; type: 'string' | 'number' }[]
> = {
  local: [
    {
      key: 'root_path',
      label: i18nText('settings', 'auto.root_directory_path'),
      type: 'string'
    },
    {
      key: 'public_base_url',
      label: i18nText('settings', 'auto.public_base_url'),
      type: 'string'
    }
  ],
  s3: [
    { key: 'endpoint', label: 'Endpoint', type: 'string' },
    { key: 'region', label: 'Region', type: 'string' },
    { key: 'bucket', label: 'Bucket', type: 'string' },
    { key: 'access_key_id', label: 'Access Key ID', type: 'string' },
    { key: 'secret_access_key', label: 'Secret Access Key', type: 'string' },
    { key: 'force_path_style', label: 'Force Path Style', type: 'string' }
  ],
  oss: [
    { key: 'endpoint', label: 'Endpoint', type: 'string' },
    { key: 'region', label: 'Region', type: 'string' },
    { key: 'bucket', label: 'Bucket', type: 'string' },
    { key: 'access_key_id', label: 'Access Key ID', type: 'string' },
    { key: 'secret_access_key', label: 'Secret Access Key', type: 'string' }
  ],
  cos: [
    { key: 'endpoint', label: 'Endpoint', type: 'string' },
    { key: 'region', label: 'Region', type: 'string' },
    { key: 'bucket', label: 'Bucket', type: 'string' },
    { key: 'access_key_id', label: 'Access Key ID', type: 'string' },
    { key: 'secret_access_key', label: 'Secret Access Key', type: 'string' }
  ],
  rustfs: [
    { key: 'endpoint', label: 'Endpoint', type: 'string' },
    { key: 'region', label: 'Region', type: 'string' },
    { key: 'bucket', label: 'Bucket', type: 'string' },
    { key: 'access_key_id', label: 'Access Key ID', type: 'string' },
    { key: 'secret_access_key', label: 'Secret Access Key', type: 'string' },
    { key: 'force_path_style', label: 'Force Path Style', type: 'string' },
    {
      key: 'public_base_url',
      label: i18nText('settings', 'auto.public_base_url'),
      type: 'string'
    }
  ]
};

function toStorageFormObject(
  value: Record<string, unknown> | null | undefined
): StorageFormObject {
  return (value ?? {}) as StorageFormObject;
}

export function FileStorageDrawer({
  open,
  mode,
  record,
  onClose,
  onSuccess
}: FileStorageDrawerProps) {
  const { message } = App.useApp();
  const [form] = Form.useForm<StorageFormValues>();
  const [submitting, setSubmitting] = useState(false);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const isView = mode === 'view';

  useEffect(() => {
    if (open) {
      if (record && mode !== 'create') {
        form.setFieldsValue({
          code: record.code,
          title: record.title,
          driver_type: record.driver_type,
          enabled: record.enabled,
          is_default: record.is_default,
          config_json: toStorageFormObject(record.config_json),
          rule_json: toStorageFormObject(record.rule_json)
        });
      } else {
        form.resetFields();
      }
    }
  }, [open, record, mode, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);

      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      const input: CreateSettingsFileStorageInput = {
        code: values.code,
        title: values.title,
        driver_type: values.driver_type,
        enabled: values.enabled,
        is_default: values.is_default,
        config_json: values.config_json ?? {},
        rule_json: values.rule_json ?? {}
      };

      if (mode === 'edit' && record) {
        const updateInput: UpdateSettingsFileStorageInput = {
          title: input.title,
          enabled: input.enabled,
          is_default: input.is_default,
          config_json: input.config_json,
          rule_json: input.rule_json
        };
        await updateSettingsFileStorage(record.id, updateInput, csrfToken);
        message.success(
          i18nText('settings', 'auto.storage_configuration_updated')
        );
      } else {
        await createSettingsFileStorage(input, csrfToken);
        message.success(
          i18nText('settings', 'auto.storage_configuration_created')
        );
      }

      onSuccess();
      onClose();
    } catch (err: unknown) {
      if (err && typeof err === 'object' && 'errorFields' in err) return;
      const msg =
        err instanceof Error
          ? err.message
          : i18nText('settings', 'auto.operation_failed_retry');
      message.error(msg);
    } finally {
      setSubmitting(false);
    }
  };

  const currentDriver = Form.useWatch('driver_type', form);

  return (
    <Drawer
      title={
        mode === 'create'
          ? i18nText('settings', 'auto.add_storage_configuration')
          : mode === 'edit'
            ? i18nText('settings', 'auto.edit_storage_configuration')
            : i18nText('settings', 'auto.view_storage_configuration')
      }
      open={open}
      onClose={onClose}
      size={520}
      extra={
        !isView ? (
          <Button type="primary" loading={submitting} onClick={handleSubmit}>
            {mode === 'create'
              ? i18nText('settings', 'auto.create')
              : i18nText('settings', 'auto.save')}
          </Button>
        ) : undefined
      }
    >
      <Form
        form={form}
        layout="vertical"
        disabled={isView}
        initialValues={{
          driver_type: 'local',
          enabled: true,
          is_default: false,
          config_json: {},
          rule_json: {}
        }}
      >
        <Form.Item
          name="code"
          label={i18nText('settings', 'auto.storage_code')}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.storage_code_required')
            }
          ]}
        >
          <Input
            placeholder={i18nText('settings', 'auto.storage_code_placeholder')}
            disabled={mode === 'edit' || isView}
          />
        </Form.Item>

        <Form.Item
          name="title"
          label={i18nText('settings', 'auto.name')}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.name_required')
            }
          ]}
        >
          <Input
            placeholder={i18nText('settings', 'auto.storage_name_placeholder')}
          />
        </Form.Item>

        <Form.Item
          name="driver_type"
          label={i18nText('settings', 'auto.driver_type')}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.driver_type_required')
            }
          ]}
        >
          <Select
            options={DRIVER_TYPE_OPTIONS}
            disabled={mode === 'edit' || isView}
          />
        </Form.Item>

        <Form.Item
          name="enabled"
          label={i18nText('settings', 'auto.enabled')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        <Form.Item
          name="is_default"
          label={i18nText('settings', 'auto.set_as_default_storage')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        {currentDriver && DRIVER_FIELDS[currentDriver] && (
          <div className="storage-drawer-driver-config">
            <h4>{i18nText('settings', 'auto.driver_configuration')}</h4>
            {DRIVER_FIELDS[currentDriver].map((field) => (
              <Form.Item
                key={field.key}
                name={['config_json', field.key]}
                label={field.label}
              >
                {field.type === 'number' ? (
                  <InputNumber style={{ width: '100%' }} />
                ) : (
                  <Input
                    placeholder={i18nText(
                      'settings',
                      'auto.field_placeholder',
                      { value1: field.label }
                    )}
                  />
                )}
              </Form.Item>
            ))}
          </div>
        )}

        <Form.Item
          name={['rule_json', 'description']}
          label={i18nText('settings', 'auto.rule_description')}
        >
          <Input.TextArea
            rows={2}
            placeholder={i18nText('settings', 'auto.optional')}
          />
        </Form.Item>
      </Form>
    </Drawer>
  );
}
