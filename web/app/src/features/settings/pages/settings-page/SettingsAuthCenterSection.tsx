import { useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Drawer,
  Flex,
  Form,
  Input,
  InputNumber,
  Space,
  Switch,
  Table,
  Tag
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import {
  enableSettingsAuthCenterAuthenticator,
  fetchSettingsAuthCenterOverview,
  settingsAuthCenterOverviewQueryKey,
  type SettingsAuthCenterOverview
} from '../../api/auth-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

type AuthenticatorRow = SettingsAuthCenterOverview['authenticators'][number];

function isPrimitiveConfigValue(
  value: unknown
): value is string | number | boolean | null {
  return (
    value == null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  );
}

function authCenterConfigFormValues(row: AuthenticatorRow) {
  // @field-contract-compat source=ConsoleAuthCenterAuthenticator.config_values alias=missing remove_by=2026-07-31
  const configValues = row.config_values ?? {};

  return Object.fromEntries(
    Object.entries(configValues).filter(([, value]) =>
      isPrimitiveConfigValue(value)
    )
  );
}

function authCenterConfigSchema(row: AuthenticatorRow) {
  // @field-contract-compat source=ConsoleAuthCenterAuthenticator.config_schema alias=missing remove_by=2026-07-31
  return row.config_schema ?? [];
}

function AuthenticatorConfigDrawer({
  authenticator,
  open,
  onClose
}: {
  authenticator: AuthenticatorRow | null;
  open: boolean;
  onClose: () => void;
}) {
  const configValues = authenticator
    ? authCenterConfigFormValues(authenticator)
    : {};
  const configSchema = authenticator
    ? authCenterConfigSchema(authenticator)
    : [];

  return (
    <Drawer
      title={
        authenticator
          ? i18nText('settings', 'auto.configuration', {
              value1: authenticator.title
            })
          : i18nText('settings', 'auto.configuration_alt')
      }
      width={520}
      open={open}
      onClose={onClose}
      destroyOnClose
    >
      {authenticator ? (
        <Space
          direction="vertical"
          size={16}
          className="settings-auth-center-drawer"
        >
          <Descriptions size="small" column={1}>
            <Descriptions.Item label={i18nText('settings', 'auto.name')}>
              {authenticator.name}
            </Descriptions.Item>
            <Descriptions.Item label={i18nText('settings', 'auto.kind')}>
              {authenticator.auth_type}
            </Descriptions.Item>
          </Descriptions>
          <Form layout="vertical" disabled initialValues={configValues}>
            {configSchema.length > 0 ? (
              configSchema.map((field) => (
                <Form.Item
                  key={field.key}
                  name={field.key}
                  label={field.label}
                  valuePropName={field.type === 'boolean' ? 'checked' : 'value'}
                >
                  {field.type === 'boolean' ? (
                    <Switch />
                  ) : field.type === 'number' ? (
                    <InputNumber className="settings-auth-center-drawer__number" />
                  ) : (
                    <Input />
                  )}
                </Form.Item>
              ))
            ) : (
              <Descriptions size="small" column={1}>
                <Descriptions.Item
                  label={i18nText('settings', 'auto.configuration_alt')}
                >
                  {i18nText('settings', 'auto.no')}
                </Descriptions.Item>
              </Descriptions>
            )}
          </Form>
        </Space>
      ) : null}
    </Drawer>
  );
}

export function SettingsAuthCenterSection() {
  const [selectedAuthenticator, setSelectedAuthenticator] =
    useState<AuthenticatorRow | null>(null);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const queryClient = useQueryClient();
  const canManageAuthenticators =
    actor?.effective_display_role === 'root' ||
    (me?.permissions ?? []).includes('user.manage.all');
  const overviewQuery = useQuery({
    queryKey: settingsAuthCenterOverviewQueryKey,
    queryFn: fetchSettingsAuthCenterOverview
  });
  const enableMutation = useMutation({
    mutationFn: (authenticatorName: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return enableSettingsAuthCenterAuthenticator(
        authenticatorName,
        csrfToken
      );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    }
  });
  const authenticatorColumns: ColumnsType<AuthenticatorRow> = [
    {
      title: i18nText('settings', 'auto.name'),
      dataIndex: 'name',
      key: 'name'
    },
    {
      title: i18nText('settings', 'auto.kind'),
      dataIndex: 'auth_type',
      key: 'auth_type'
    },
    {
      title: i18nText('settings', 'auto.title'),
      dataIndex: 'title',
      key: 'title'
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'enabled',
      key: 'enabled',
      render: (enabled: boolean) => (
        <Tag color={enabled ? 'green' : 'default'}>
          {i18nText('settings', enabled ? 'auto.enabled_alt' : 'auto.disabled')}
        </Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.built_in'),
      dataIndex: 'is_builtin',
      key: 'is_builtin',
      render: (isBuiltin: boolean) =>
        isBuiltin
          ? i18nText('settings', 'auto.yes')
          : i18nText('settings', 'auto.no')
    },
    {
      title: i18nText('settings', 'auto.enabled'),
      dataIndex: 'enabled',
      key: 'enable',
      render: (enabled: boolean, row) => (
        <Switch
          checked={enabled}
          disabled={enabled || !csrfToken || !canManageAuthenticators}
          loading={
            enableMutation.isPending && enableMutation.variables === row.name
          }
          onChange={(checked) => {
            if (checked && !enabled) {
              enableMutation.mutate(row.name);
            }
          }}
        />
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'operation',
      render: (_, row) => (
        <Space size="small">
          <Button
            type="link"
            size="small"
            onClick={() => setSelectedAuthenticator(row)}
          >
            {i18nText('settings', 'auto.edit')}
          </Button>
        </Space>
      )
    }
  ];

  return (
    <SettingsSectionSurface
      title={i18nText('settings', 'auto.auth_center')}
      hideHeader
      heightMode="fill"
    >
      {overviewQuery.isLoading ? <LoadingState compact /> : null}
      {overviewQuery.isError ? (
        <Alert
          type="error"
          message={i18nText(
            'settings',
            'auto.auth_center_overview_load_failed'
          )}
        />
      ) : null}
      {overviewQuery.data ? (
        <Flex vertical gap="large">
          <Table
            rowKey="name"
            columns={authenticatorColumns}
            dataSource={overviewQuery.data.authenticators}
            pagination={false}
            size="middle"
          />
        </Flex>
      ) : null}
      <AuthenticatorConfigDrawer
        authenticator={selectedAuthenticator}
        open={selectedAuthenticator != null}
        onClose={() => setSelectedAuthenticator(null)}
      />
    </SettingsSectionSurface>
  );
}
