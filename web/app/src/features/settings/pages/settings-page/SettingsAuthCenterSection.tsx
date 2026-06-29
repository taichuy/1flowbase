import { useEffect, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Drawer,
  Flex,
  Form,
  Input,
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
  updateSettingsAuthCenterAuthenticatorConfig,
  type SettingsAuthCenterOverview
} from '../../api/auth-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

import './auth-center-panel.css';

type AuthenticatorRow = SettingsAuthCenterOverview['authenticators'][number];

type AuthenticatorConfigFormValues = {
  name: string;
  title: string;
  enabled: boolean;
  description: string | null;
};

function authCenterConfigFormValues(
  row: AuthenticatorRow
): AuthenticatorConfigFormValues {
  const description =
    typeof row.config_values.description === 'string'
      ? row.config_values.description
      : null;

  return {
    name: row.name,
    title: row.title,
    enabled: row.enabled,
    description
  };
}

function AuthenticatorConfigDrawer({
  authenticator,
  open,
  canManageAuthenticators,
  hasCsrfToken,
  submitting,
  errorMessage,
  onClose,
  onSubmit
}: {
  authenticator: AuthenticatorRow | null;
  open: boolean;
  canManageAuthenticators: boolean;
  hasCsrfToken: boolean;
  submitting: boolean;
  errorMessage: string | null;
  onClose: () => void;
  onSubmit: (
    authenticatorName: string,
    values: AuthenticatorConfigFormValues
  ) => void;
}) {
  const [form] = Form.useForm<AuthenticatorConfigFormValues>();
  const accessErrorMessage = !canManageAuthenticators
    ? i18nText('settings', 'auto.auth_center_manage_permission_required')
    : !hasCsrfToken
      ? i18nText('settings', 'auto.auth_center_csrf_required')
      : null;

  useEffect(() => {
    if (authenticator) {
      form.setFieldsValue(authCenterConfigFormValues(authenticator));
    } else {
      form.resetFields();
    }
  }, [authenticator, form]);

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
      footer={
        authenticator ? (
          <Flex justify="start" gap="small">
            <Button
              type="primary"
              loading={submitting}
              disabled={accessErrorMessage != null}
              onClick={() => form.submit()}
            >
              {i18nText('settings', 'auto.save')}
            </Button>
            <Button onClick={onClose}>
              {i18nText('settings', 'auto.cancel')}
            </Button>
          </Flex>
        ) : null
      }
    >
      {authenticator ? (
        <Space
          direction="vertical"
          size={16}
          className="settings-auth-center-drawer"
        >
          {accessErrorMessage ? (
            <Alert type="error" showIcon message={accessErrorMessage} />
          ) : null}
          {errorMessage ? (
            <Alert type="error" showIcon message={errorMessage} />
          ) : null}
          <Form
            form={form}
            layout="vertical"
            onFinish={(values) => onSubmit(authenticator.name, values)}
          >
            <Form.Item
              name="name"
              label={i18nText('settings', 'auto.identifier')}
            >
              <Input disabled readOnly />
            </Form.Item>
            <Form.Item
              name="title"
              label={i18nText('settings', 'auto.name')}
              rules={[
                {
                  required: true,
                  message: i18nText('settings', 'auto.please_fill_in', {
                    value1: i18nText('settings', 'auto.name')
                  })
                }
              ]}
            >
              <Input disabled={!canManageAuthenticators || !hasCsrfToken} />
            </Form.Item>
            <Form.Item
              name="description"
              label={i18nText('settings', 'auto.description')}
            >
              <Input.TextArea
                rows={4}
                disabled={!canManageAuthenticators || !hasCsrfToken}
              />
            </Form.Item>
            <Form.Item
              name="enabled"
              label={i18nText('settings', 'auto.enabled')}
              valuePropName="checked"
            >
              <Switch disabled={!canManageAuthenticators || !hasCsrfToken} />
            </Form.Item>
          </Form>
        </Space>
      ) : null}
    </Drawer>
  );
}

export function SettingsAuthCenterSection() {
  const [selectedAuthenticatorName, setSelectedAuthenticatorName] = useState<
    string | null
  >(null);
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
  const selectedAuthenticator =
    selectedAuthenticatorName && overviewQuery.data
      ? (overviewQuery.data.authenticators.find(
          (row) => row.name === selectedAuthenticatorName
        ) ?? null)
      : null;
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
  const configMutation = useMutation({
    mutationFn: (input: {
      authenticatorName: string;
      values: AuthenticatorConfigFormValues;
    }) => {
      if (!canManageAuthenticators) {
        throw new Error(
          i18nText('settings', 'auto.auth_center_manage_permission_required')
        );
      }
      if (!csrfToken) {
        throw new Error(i18nText('settings', 'auto.auth_center_csrf_required'));
      }
      return updateSettingsAuthCenterAuthenticatorConfig(
        input.authenticatorName,
        {
          name: input.values.name,
          title: input.values.title,
          enabled: input.values.enabled,
          description: input.values.description
        },
        csrfToken
      );
    },
    onSuccess: async (authenticator) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                authenticators: overview.authenticators.map((row) =>
                  row.name === authenticator.name ? authenticator : row
                )
              }
            : overview
      );
      setSelectedAuthenticatorName(null);
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    }
  });
  const authenticatorColumns: ColumnsType<AuthenticatorRow> = [
    {
      title: i18nText('settings', 'auto.identifier'),
      dataIndex: 'name',
      key: 'name'
    },
    {
      title: i18nText('settings', 'auto.name'),
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
            onClick={() => {
              configMutation.reset();
              setSelectedAuthenticatorName(row.name);
            }}
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
        open={selectedAuthenticatorName != null}
        canManageAuthenticators={canManageAuthenticators}
        hasCsrfToken={csrfToken != null}
        submitting={configMutation.isPending}
        errorMessage={
          configMutation.isError
            ? i18nText('settings', 'auto.auth_center_config_update_failed')
            : null
        }
        onClose={() => {
          configMutation.reset();
          setSelectedAuthenticatorName(null);
        }}
        onSubmit={(authenticatorName, values) => {
          configMutation.mutate({ authenticatorName, values });
        }}
      />
    </SettingsSectionSurface>
  );
}
