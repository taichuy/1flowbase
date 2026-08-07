import { useEffect, useMemo, useState } from 'react';

import {
  Checkbox,
  Descriptions,
  Drawer,
  Form,
  Input,
  InputNumber,
  Button,
  Select,
  Space,
  Switch
} from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import { useAuthStore } from '../../../../state/auth-store';
import {
  saveSettingsHostInfrastructureProviderConfig,
  settingsHostInfrastructureProvidersQueryKey,
  type SettingsHostInfrastructureProviderConfig
} from '../../api/host-infrastructure';
import { i18nText } from '../../../../shared/i18n/text';

type ConfigFieldValue = string | number | boolean | null | undefined;
type ConfigValues = Record<string, ConfigFieldValue>;

function isConfigFieldValue(value: unknown): value is ConfigFieldValue {
  return (
    value == null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  );
}

function compactConfig(values: ConfigValues): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(values).filter(([, value]) => value !== undefined)
  );
}

function compactInitialConfig(values: Record<string, unknown>): ConfigValues {
  return Object.fromEntries(
    Object.entries(values).filter(([, value]) => isConfigFieldValue(value))
  ) as ConfigValues;
}

function defaultValuesFromSchema(
  provider: SettingsHostInfrastructureProviderConfig
): ConfigValues {
  return Object.fromEntries(
    provider.config_schema
      .filter((field) => isConfigFieldValue(field.default_value))
      .map((field) => [field.key, field.default_value])
  ) as ConfigValues;
}

export function HostInfrastructureProviderDrawer({
  provider,
  canManage,
  open,
  onSaved,
  onClose
}: {
  provider: SettingsHostInfrastructureProviderConfig | null;
  canManage: boolean;
  open: boolean;
  onSaved: () => void;
  onClose: () => void;
}) {
  const [form] = Form.useForm<ConfigValues>();
  const [enabledContracts, setEnabledContracts] = useState<string[]>([]);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const saveMutation = useMutation({
    mutationFn: async (values: ConfigValues) => {
      if (!provider || !csrfToken) {
        return null;
      }
      return saveSettingsHostInfrastructureProviderConfig(
        provider.installation_id,
        provider.provider_code,
        {
          enabled_contracts: enabledContracts,
          config_json: compactConfig(values)
        },
        csrfToken
      );
    },
    onSuccess: async (result) => {
      if (!result) {
        return;
      }
      await queryClient.invalidateQueries({
        queryKey: settingsHostInfrastructureProvidersQueryKey
      });
      onSaved();
      onClose();
    }
  });
  const initialValues = useMemo(() => {
    if (!provider) {
      return {};
    }
    return {
      ...defaultValuesFromSchema(provider),
      ...compactInitialConfig(provider.config_json)
    };
  }, [provider]);

  useEffect(() => {
    if (!provider) {
      return;
    }
    form.setFieldsValue(initialValues);
    setEnabledContracts(provider.enabled_contracts);
  }, [form, initialValues, provider]);

  return (
    <>
      <Drawer
        title={provider ? i18nText("settings", "auto.configuration", { value1: provider.display_name }) : i18nText("settings", "auto.provider_configuration")}
        size={520}
        open={open}
        onClose={onClose}
        destroyOnHidden
        extra={
          <Space>
            <Button
              type="primary"
              htmlType="submit"
              form="host-infrastructure-provider-form"
              disabled={!canManage || saveMutation.isPending}
              loading={saveMutation.isPending}
            >
              {i18nText("settings", "auto.save_wait_restart")}</Button>
          </Space>
        }
      >
        {provider ? (
          <Space orientation="vertical" size={16} className="host-infrastructure-drawer">
            <Descriptions size="small" column={1}>
              <Descriptions.Item label="Extension">
                {provider.extension_id}
              </Descriptions.Item>
              <Descriptions.Item label="Provider">
                {provider.provider_code}
              </Descriptions.Item>
              <Descriptions.Item label="Config">
                {provider.config_ref}
              </Descriptions.Item>
            </Descriptions>
            <Checkbox.Group
              value={enabledContracts}
              onChange={(values) => setEnabledContracts(values.map(String))}
              disabled={!canManage}
              options={provider.contracts.map((contract) => ({
                label: contract,
                value: contract
              }))}
            />
            <Form
              id="host-infrastructure-provider-form"
              form={form}
              layout="vertical"
              initialValues={initialValues}
              onFinish={(values) => saveMutation.mutate(values)}
              disabled={!canManage}
            >
              {provider.config_schema.map((field) => (
                <Form.Item
                  key={field.key}
                  name={field.key}
                  label={field.label}
                  valuePropName={field.type === 'boolean' ? 'checked' : 'value'}
                  rules={[
                    {
                      required: field.required,
                      message: i18nText("settings", "auto.cannot_be_empty", { value1: field.label })
                    }
                  ]}
                >
                  {field.type === 'number' ? (
                    <InputNumber
                      min={field.min}
                      max={field.max}
                      step={field.step}
                      precision={field.precision}
                      className="host-infrastructure-drawer__number"
                    />
                  ) : field.type === 'boolean' ? (
                    <Switch />
                  ) : field.type === 'select' ? (
                    <Select
                      options={field.options.map((option) => ({
                        label: option.label,
                        value: option.value as string | number,
                        disabled: option.disabled
                      }))}
                    />
                  ) : (
                    <Input
                      placeholder={
                        field.send_mode === 'secret_ref'
                          ? 'env://REDIS_PASSWORD'
                          : field.placeholder
                      }
                    />
                  )}
                </Form.Item>
              ))}
            </Form>
          </Space>
        ) : null}
      </Drawer>
    </>
  );
}
