import { DeleteOutlined, PlusOutlined, SaveOutlined } from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Form,
  Input,
  InputNumber,
  Col,
  Row,
  Space,
  Switch,
  Typography,
  message
} from 'antd';
import { useEffect, useMemo, useRef } from 'react';
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
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import { JsonProtocolInlineEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonProtocolInlineEditor';
import { stringifyJson } from './mcp-management-utils';

type DiscoveryPolicyFormValues = Omit<
  ConsoleMcpInstanceDiscoveryPolicy,
  | 'id'
  | 'workspace_id'
  | 'instance_record_id'
  | 'instance_id'
  | 'list_return_fields'
> & {
  list_return_fields: string[];
};

function parseReturnFields(value: string) {
  try {
    const parsed = JSON.parse(value) as unknown;

    if (
      !Array.isArray(parsed) ||
      parsed.some((field) => typeof field !== 'string')
    ) {
      return {
        ok: false as const,
        message: i18nText(
          'settingsMcpManagement',
          'auto.list_return_fields_array_error'
        )
      };
    }

    return { ok: true as const, value: parsed };
  } catch {
    return {
      ok: false as const,
      message: i18nText(
        'settingsMcpManagement',
        'auto.list_return_fields_json_error'
      )
    };
  }
}

function ReturnFieldsEditor({
  value = [],
  onChange,
  onValidityChange
}: {
  value?: string[];
  onChange?: (value: string[]) => void;
  onValidityChange: (valid: boolean) => void;
}) {
  const fieldLabel = i18nText(
    'settingsMcpManagement',
    'auto.list_return_fields'
  );

  return (
    <JsonProtocolInlineEditor
      ariaLabel={`${fieldLabel} JSON`}
      className="mcp-management__return-fields-editor"
      hint={i18nText(
        'settingsMcpManagement',
        'auto.list_return_fields_json_hint'
      )}
      parseValue={parseReturnFields}
      stringifyValue={stringifyJson}
      value={value}
      onChange={(nextValue) => onChange?.(nextValue)}
      onValidityChange={onValidityChange}
      renderFields={({ value: fields, onChange: setFields }) => (
        <div className="mcp-management__return-fields">
          <div className="mcp-management__return-field-rows">
            {fields.map((field, index) => (
              <div key={index} className="mcp-management__return-field-row">
                <Input
                  aria-label={`${fieldLabel} ${index + 1}`}
                  value={field}
                  onChange={(event) => {
                    const nextFields = [...fields];
                    nextFields[index] = event.target.value;
                    setFields(nextFields);
                  }}
                />
                <Button
                  danger
                  type="text"
                  aria-label={i18nText(
                    'settingsMcpManagement',
                    'auto.delete_list_return_field',
                    { value1: field || index + 1 }
                  )}
                  icon={<DeleteOutlined />}
                  onClick={() =>
                    setFields(
                      fields.filter((_, fieldIndex) => fieldIndex !== index)
                    )
                  }
                />
              </div>
            ))}
          </div>
          <Button
            block
            type="dashed"
            icon={<PlusOutlined />}
            onClick={() => setFields([...fields, ''])}
          >
            {i18nText('settingsMcpManagement', 'auto.add_list_return_field')}
          </Button>
        </div>
      )}
    />
  );
}

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
  const returnFieldsValidRef = useRef(true);
  const initialValues = useMemo<DiscoveryPolicyFormValues>(
    () => ({
      list_default_limit: policy.list_default_limit,
      list_max_depth: policy.list_max_depth,
      list_regex_enabled: policy.list_regex_enabled,
      list_regex_max_length: policy.list_regex_max_length,
      list_return_fields: policy.list_return_fields as string[]
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
          list_return_fields: values.list_return_fields
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
    <FixedHeightModal
      open={open}
      title={`${i18nText('settingsMcpManagement', 'auto.discovery_policy')} · ${instance.name}`}
      onCancel={onClose}
      footer={
        <Space>
          <Button onClick={onClose}>
            {i18nText('settings', 'auto.cancel')}
          </Button>
          <Button
            type="primary"
            icon={<SaveOutlined />}
            disabled={!canManage}
            loading={saveMutation.isPending}
            onClick={() => {
              if (!returnFieldsValidRef.current) {
                message.error(
                  i18nText(
                    'settingsMcpManagement',
                    'auto.list_return_fields_json_error'
                  )
                );
                return;
              }

              form.submit();
            }}
          >
            {i18nText('settings', 'auto.save')}
          </Button>
        </Space>
      }
      destroyOnHidden
    >
      <Space
        orientation="vertical"
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
          <Row gutter={24}>
            <Col xs={24} sm={12}>
              <Form.Item
                name="list_default_limit"
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.list_default_limit'
                )}
                rules={[{ required: true }]}
              >
                <InputNumber min={1} />
              </Form.Item>
            </Col>
            <Col xs={24} sm={12}>
              <Form.Item
                name="list_max_depth"
                label={i18nText('settingsMcpManagement', 'auto.list_max_depth')}
                rules={[{ required: true }]}
              >
                <InputNumber min={1} />
              </Form.Item>
            </Col>
          </Row>
          <Row gutter={24}>
            <Col xs={24} sm={12}>
              <Form.Item
                name="list_regex_enabled"
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.list_regex_enabled'
                )}
                valuePropName="checked"
              >
                <Switch />
              </Form.Item>
            </Col>
            <Col xs={24} sm={12}>
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
            </Col>
          </Row>
          <Form.Item
            name="list_return_fields"
            label={i18nText('settingsMcpManagement', 'auto.list_return_fields')}
            required
          >
            <ReturnFieldsEditor
              onValidityChange={(valid) => {
                returnFieldsValidRef.current = valid;
              }}
            />
          </Form.Item>
        </Form>
      </Space>
    </FixedHeightModal>
  );
}
