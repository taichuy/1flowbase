import { useEffect, useMemo, useRef, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Space,
  Switch,
  Table,
  Tag
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  SchemaFormDrawer,
  type SchemaFormValues
} from '../../../../shared/schema-ui/form-drawer/SchemaFormDrawer';
import type { PluginFormSchema } from '../../../../shared/schema-ui/contracts/plugin-form-schema';
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

const DEFAULT_AUTH_CENTER_DRAWER_WIDTH = 520;
const MIN_AUTH_CENTER_DRAWER_WIDTH = 480;
const MAX_AUTH_CENTER_DRAWER_WIDTH = 960;
const KEYBOARD_RESIZE_STEP = 40;

type AuthenticatorConfigFormValues = {
  name: string;
  title: string;
  enabled: boolean;
  description: string | null;
};

function clampAuthCenterDrawerWidth(width: number) {
  return Math.min(
    MAX_AUTH_CENTER_DRAWER_WIDTH,
    Math.max(MIN_AUTH_CENTER_DRAWER_WIDTH, width)
  );
}

function authCenterConfigFormValues(
  row: AuthenticatorRow
): SchemaFormValues {
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

function authCenterConfigFormSchema(): PluginFormSchema {
  return {
    schema_version: '1.0.0',
    fields: [
      {
        key: 'name',
        label: i18nText('settings', 'auto.identifier'),
        type: 'string',
        read_only: true
      },
      {
        key: 'title',
        label: i18nText('settings', 'auto.name'),
        type: 'string',
        required: true
      },
      {
        key: 'description',
        label: i18nText('settings', 'auto.description'),
        type: 'string',
        control: 'textarea'
      },
      {
        key: 'enabled',
        label: i18nText('settings', 'auto.enabled'),
        type: 'boolean'
      }
    ]
  };
}

function toAuthenticatorConfigFormValues(
  values: SchemaFormValues
): AuthenticatorConfigFormValues {
  return {
    name: String(values.name ?? ''),
    title: String(values.title ?? ''),
    enabled: Boolean(values.enabled),
    description:
      typeof values.description === 'string' ? values.description : null
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
  ) => Promise<void>;
}) {
  const [drawerWidth, setDrawerWidth] = useState(
    DEFAULT_AUTH_CENTER_DRAWER_WIDTH
  );
  const dragStartRef = useRef<{ pointerX: number; width: number } | null>(
    null
  );
  const accessErrorMessage = !canManageAuthenticators
    ? i18nText('settings', 'auto.auth_center_manage_permission_required')
    : !hasCsrfToken
      ? i18nText('settings', 'auto.auth_center_csrf_required')
      : null;
  const initialValues = useMemo(
    () => (authenticator ? authCenterConfigFormValues(authenticator) : {}),
    [authenticator]
  );
  const schema = useMemo(() => authCenterConfigFormSchema(), []);

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = dragStartRef.current;
      if (!dragStart) {
        return;
      }

      setDrawerWidth(
        clampAuthCenterDrawerWidth(
          dragStart.width + dragStart.pointerX - event.clientX
        )
      );
    };

    const handleMouseUp = () => {
      dragStartRef.current = null;
      document.body.classList.remove('settings-auth-center--resizing-drawer');
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.classList.remove('settings-auth-center--resizing-drawer');
    };
  }, []);

  const statusMessages = [
    accessErrorMessage
      ? {
          key: 'access-error',
          message: accessErrorMessage,
          type: 'error' as const
        }
      : null,
    errorMessage
      ? {
          key: 'submit-error',
          message: errorMessage,
          type: 'error' as const
        }
      : null
  ].filter((message) => message != null);

  return authenticator ? (
    <SchemaFormDrawer
      bodyClassName="settings-auth-center-drawer"
      disabled={accessErrorMessage != null}
      initialValues={initialValues}
      open={open}
      rootClassName="settings-auth-center-drawer-shell"
      schema={schema}
      statusMessages={statusMessages}
      submitting={submitting}
      title={
        i18nText('settings', 'auto.configuration', {
          value1: authenticator.title
        })
      }
      width={drawerWidth}
      leadingContent={
          <div
            aria-label="调整认证器配置抽屉宽度"
            aria-orientation="vertical"
            aria-valuemax={MAX_AUTH_CENTER_DRAWER_WIDTH}
            aria-valuemin={MIN_AUTH_CENTER_DRAWER_WIDTH}
            aria-valuenow={drawerWidth}
            className="settings-auth-center-drawer__resize-handle"
            role="separator"
            tabIndex={0}
            onKeyDown={(event) => {
              if (event.key === 'ArrowLeft') {
                event.preventDefault();
                setDrawerWidth((currentWidth) =>
                  clampAuthCenterDrawerWidth(
                    currentWidth + KEYBOARD_RESIZE_STEP
                  )
                );
                return;
              }

              if (event.key === 'ArrowRight') {
                event.preventDefault();
                setDrawerWidth((currentWidth) =>
                  clampAuthCenterDrawerWidth(
                    currentWidth - KEYBOARD_RESIZE_STEP
                  )
                );
                return;
              }

              if (event.key === 'Home') {
                event.preventDefault();
                setDrawerWidth(MIN_AUTH_CENTER_DRAWER_WIDTH);
                return;
              }

              if (event.key === 'End') {
                event.preventDefault();
                setDrawerWidth(MAX_AUTH_CENTER_DRAWER_WIDTH);
              }
            }}
            onMouseDown={(event) => {
              event.preventDefault();
              dragStartRef.current = {
                pointerX: event.clientX,
                width: drawerWidth
              };
              document.body.classList.add(
                'settings-auth-center--resizing-drawer'
              );
            }}
          />
      }
      onCancel={onClose}
      onSubmit={(values) =>
        onSubmit(authenticator.name, toAuthenticatorConfigFormValues(values))
      }
    />
  ) : null;
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
      title: i18nText('settings', 'auto.auth_center_description'),
      key: 'description',
      render: (_, row) => row.config_values.description || '-'
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
        onSubmit={async (authenticatorName, values) => {
          await new Promise<void>((resolve, reject) => {
            configMutation.mutate(
              { authenticatorName, values },
              {
                onError: () =>
                  reject(
                    new Error(
                      i18nText(
                        'settings',
                        'auto.auth_center_config_update_failed'
                      )
                    )
                  ),
                onSuccess: () => resolve()
              }
            );
          });
        }}
      />
    </SettingsSectionSurface>
  );
}
