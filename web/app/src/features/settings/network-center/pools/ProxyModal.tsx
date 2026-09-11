import { Alert, Button, Form, Input, Select, Skeleton, Space } from 'antd';

import type {
  CreateSettingsNetworkEgressProxyInput,
  SettingsNetworkEgressProviderType,
  SettingsNetworkEgressProxy
} from '../../api/network-center';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';

type Props = {
  mode: 'create' | 'edit';
  proxy?: SettingsNetworkEgressProxy;
  types: SettingsNetworkEgressProviderType[];
  loading: boolean;
  error: boolean;
  submitting: boolean;
  onRetry: () => void;
  onClose: () => void;
  onSubmit: (values: CreateSettingsNetworkEgressProxyInput) => void;
};

export function ProxyModal({
  mode,
  proxy,
  types,
  loading,
  error,
  submitting,
  onRetry,
  onClose,
  onSubmit
}: Props) {
  const [form] = Form.useForm<CreateSettingsNetworkEgressProxyInput>();
  const providerCode = Form.useWatch('provider_code', form);
  const schema =
    mode === 'edit'
      ? proxy?.form_schema
      : types.find((type) => type.provider_code === providerCode)?.form_schema;
  const configuredSecrets =
    mode === 'edit' ? (proxy?.configured_secret_fields ?? []) : [];
  return (
    <FixedHeightModal
      open
      title={i18nText(
        'settings',
        mode === 'edit'
          ? 'auto.network_center_member_edit'
          : 'auto.network_center_member_create'
      )}
      onCancel={onClose}
      onOk={() => {
        if (!loading && !error) form.submit();
      }}
      confirmLoading={submitting}
      footer={
        <Space>
          <Button disabled={submitting} onClick={onClose}>
            {i18nText('settings', 'auto.cancel')}
          </Button>
          <Button
            type="primary"
            loading={submitting}
            disabled={loading || error}
            onClick={() => form.submit()}
          >
            {i18nText('settings', 'auto.save')}
          </Button>
        </Space>
      }
      okText={i18nText('settings', 'auto.save')}
      destroyOnHidden
      width={640}
      height="min(760px, calc(100dvh - 48px))"
    >
      {loading ? (
        <Skeleton active />
      ) : error ? (
        <Alert
          type="error"
          showIcon
          title={i18nText('settings', 'auto.network_center_proxy_load_failed')}
          action={
            <Button onClick={onRetry}>
              {i18nText('settings', 'auto.refresh')}
            </Button>
          }
        />
      ) : (
        <Form
          form={form}
          layout="vertical"
          initialValues={
            mode === 'edit'
              ? {
                  provider_code: proxy?.provider_code,
                  display_name: proxy?.display_name,
                  description: proxy?.description,
                  config: proxy?.config
                }
              : { description: '', config: {} }
          }
          onFinish={onSubmit}
          disabled={submitting}
        >
          <Form.Item
            name="provider_code"
            label={i18nText('settings', 'auto.network_center_providers')}
            rules={[{ required: true }]}
          >
            <Select
              disabled={mode === 'edit'}
              options={
                types.some(
                  (type) => type.provider_code === proxy?.provider_code
                ) || mode === 'create'
                  ? types.map((type) => ({
                      value: type.provider_code,
                      label: type.display_name
                    }))
                  : [
                      {
                        value: proxy?.provider_code,
                        label: proxy?.provider_code
                      }
                    ]
              }
              onChange={() => form.setFieldValue('config', {})}
            />
          </Form.Item>
          <Form.Item
            name="display_name"
            label={i18nText('settings', 'auto.name')}
            rules={[{ required: true, whitespace: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="description"
            label={i18nText('settings', 'auto.description')}
          >
            <Input.TextArea rows={2} />
          </Form.Item>
          {schema?.fields.map((field) => (
            <Form.Item
              key={field.key}
              name={['config', field.key]}
              label={field.label}
              extra={
                configuredSecrets.includes(field.key)
                  ? i18nText(
                      'settings',
                      'auto.network_center_proxy_secret_preserved'
                    )
                  : field.description
              }
              rules={[
                {
                  required:
                    field.required && !configuredSecrets.includes(field.key)
                }
              ]}
            >
              {field.key.toLowerCase().includes('password') ||
              configuredSecrets.includes(field.key) ? (
                <Input.Password />
              ) : (
                <Input />
              )}
            </Form.Item>
          ))}
        </Form>
      )}
    </FixedHeightModal>
  );
}
