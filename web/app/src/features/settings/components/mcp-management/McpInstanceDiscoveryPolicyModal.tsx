import { SaveOutlined } from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Form,
  Input,
  InputNumber,
  Modal,
  Space,
  Switch,
  Typography,
  message
} from 'antd';
import { useEffect, useMemo } from 'react';
import type {
  ConsoleMcpInstance,
  ConsoleMcpInstanceDiscoveryPolicy
} from '@1flowbase/api-client';

import {
  settingsMcpCatalogQueryKey,
  updateSettingsMcpInstanceDiscoveryPolicy
} from '../../api/mcp-management';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { parseJsonText, stringifyJson } from './mcp-management-utils';

type DiscoveryPolicyFormValues = Omit<
  ConsoleMcpInstanceDiscoveryPolicy,
  | 'id'
  | 'workspace_id'
  | 'instance_record_id'
  | 'instance_id'
  | 'list_return_fields'
> & {
  list_return_fields_text: string;
};

export function McpInstanceDiscoveryPolicyModal({
  canManage,
  instance,
  policy,
  open,
  onClose
}: {
  canManage: boolean;
  instance: ConsoleMcpInstance;
  policy: ConsoleMcpInstanceDiscoveryPolicy;
  open: boolean;
  onClose: () => void;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const [form] = Form.useForm<DiscoveryPolicyFormValues>();
  const initialValues = useMemo(
    () => ({
      list_default_limit: policy.list_default_limit,
      list_max_depth: policy.list_max_depth,
      list_regex_enabled: policy.list_regex_enabled,
      list_regex_max_length: policy.list_regex_max_length,
      list_return_fields_text: stringifyJson(policy.list_return_fields)
    }),
    [policy]
  );

  useEffect(() => {
    if (open) {
      form.setFieldsValue(initialValues);
    }
  }, [form, initialValues, open]);

  const saveMutation = useMutation({
    mutationFn: (values: DiscoveryPolicyFormValues) =>
      updateSettingsMcpInstanceDiscoveryPolicy(
        instance.instance_id,
        {
          list_default_limit: values.list_default_limit,
          list_max_depth: values.list_max_depth,
          list_regex_enabled: values.list_regex_enabled,
          list_regex_max_length: values.list_regex_max_length,
          list_return_fields: parseJsonText(
            values.list_return_fields_text,
            'list_return_fields'
          )
        },
        csrfToken
      ),
    onSuccess: () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      void queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
      onClose();
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });

  return (
    <Modal
      open={open}
      title={`${i18nText('settingsMcpManagement', 'auto.discovery_policy')} · ${instance.name}`}
      onCancel={onClose}
      okText={i18nText('settings', 'auto.save')}
      cancelText={i18nText('settings', 'auto.cancel')}
      okButtonProps={{
        icon: <SaveOutlined />,
        disabled: !canManage,
        loading: saveMutation.isPending
      }}
      onOk={() => form.submit()}
      destroyOnHidden
    >
      <Space
        direction="vertical"
        size="middle"
        className="mcp-management__stack"
      >
        <Typography.Text type="secondary">
          {instance.instance_id}
        </Typography.Text>
        <Form
          form={form}
          layout="vertical"
          initialValues={initialValues}
          onFinish={(values) => saveMutation.mutate(values)}
        >
          <Form.Item
            name="list_default_limit"
            label={i18nText('settingsMcpManagement', 'auto.list_default_limit')}
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
          <Form.Item
            name="list_max_depth"
            label={i18nText('settingsMcpManagement', 'auto.list_max_depth')}
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
          <Form.Item
            name="list_regex_enabled"
            label={i18nText('settingsMcpManagement', 'auto.list_regex_enabled')}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
          <Form.Item
            name="list_regex_max_length"
            label={i18nText(
              'settingsMcpManagement',
              'auto.list_regex_max_length'
            )}
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
          <Form.Item
            name="list_return_fields_text"
            label={i18nText('settingsMcpManagement', 'auto.list_return_fields')}
            rules={[{ required: true }]}
          >
            <Input.TextArea rows={5} />
          </Form.Item>
        </Form>
      </Space>
    </Modal>
  );
}
