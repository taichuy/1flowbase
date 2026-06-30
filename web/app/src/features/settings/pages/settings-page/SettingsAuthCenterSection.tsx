import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  CopyOutlined,
  DeleteOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { useEffect, useMemo, useRef, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Tooltip
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
  copySettingsAuthCenterAuthenticator,
  createSettingsAuthCenterAuthenticator,
  deleteSettingsAuthCenterAuthenticator,
  enableSettingsAuthCenterAuthenticator,
  fetchSettingsAuthCenterOverview,
  reorderSettingsAuthCenterAuthenticators,
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

type AuthenticatorLifecycleMode = 'create' | 'copy';

type AuthenticatorLifecycleModalState = {
  mode: AuthenticatorLifecycleMode;
  source: AuthenticatorRow | null;
};

type AuthenticatorLifecycleFormValues = {
  name: string;
  auth_type?: string;
  title: string;
  description?: string | null;
  enabled?: boolean;
  sort_order?: number | null;
};

function clampAuthCenterDrawerWidth(width: number) {
  return Math.min(
    MAX_AUTH_CENTER_DRAWER_WIDTH,
    Math.max(MIN_AUTH_CENTER_DRAWER_WIDTH, width)
  );
}

function authCenterConfigFormValues(row: AuthenticatorRow): SchemaFormValues {
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

function authCenterLifecycleErrorMessage(error: unknown, fallbackKey: string) {
  const code =
    error && typeof error === 'object' && 'code' in error
      ? (error as { code?: unknown }).code
      : null;
  if (code === 'authenticator_identity_bindings') {
    return i18nText(
      'settings',
      'auto.auth_center_delete_bound_authenticator_failed'
    );
  }
  if (code === 'builtin_authenticator') {
    return i18nText('settings', 'auto.auth_center_delete_builtin_failed');
  }

  return i18nText('settings', fallbackKey);
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
  const dragStartRef = useRef<{ pointerX: number; width: number } | null>(null);
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
      title={i18nText('settings', 'auto.configuration', {
        value1: authenticator.title
      })}
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
                clampAuthCenterDrawerWidth(currentWidth + KEYBOARD_RESIZE_STEP)
              );
              return;
            }

            if (event.key === 'ArrowRight') {
              event.preventDefault();
              setDrawerWidth((currentWidth) =>
                clampAuthCenterDrawerWidth(currentWidth - KEYBOARD_RESIZE_STEP)
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
  const [lifecycleForm] = Form.useForm<AuthenticatorLifecycleFormValues>();
  const [lifecycleModal, setLifecycleModal] =
    useState<AuthenticatorLifecycleModalState | null>(null);
  const [operationErrorMessage, setOperationErrorMessage] = useState<
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
  const requireAuthCenterWrite = () => {
    if (!canManageAuthenticators) {
      throw new Error(
        i18nText('settings', 'auto.auth_center_manage_permission_required')
      );
    }
    if (!csrfToken) {
      throw new Error(i18nText('settings', 'auto.auth_center_csrf_required'));
    }

    return csrfToken;
  };
  const nextSortOrder =
    (overviewQuery.data?.authenticators.at(-1)?.sort_order ?? 0) + 10;
  const openCreateAuthenticator = () => {
    setOperationErrorMessage(null);
    lifecycleForm.setFieldsValue({
      name: '',
      auth_type: overviewQuery.data?.supported_auth_types[0],
      title: '',
      description: null,
      enabled: false,
      sort_order: nextSortOrder
    });
    setLifecycleModal({ mode: 'create', source: null });
  };
  const openCopyAuthenticator = (row: AuthenticatorRow) => {
    setOperationErrorMessage(null);
    lifecycleForm.setFieldsValue({
      name: `${row.name}_copy`,
      title: `${row.title} Copy`,
      description:
        typeof row.config_values.description === 'string'
          ? row.config_values.description
          : null,
      enabled: false,
      sort_order: nextSortOrder
    });
    setLifecycleModal({ mode: 'copy', source: row });
  };
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
  const createMutation = useMutation({
    mutationFn: (values: AuthenticatorLifecycleFormValues) =>
      createSettingsAuthCenterAuthenticator(
        {
          name: values.name,
          auth_type: values.auth_type ?? '',
          title: values.title,
          description: values.description ?? null,
          enabled: Boolean(values.enabled),
          sort_order: values.sort_order ?? undefined
        },
        requireAuthCenterWrite()
      ),
    onSuccess: async (authenticator) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                authenticators: [
                  ...overview.authenticators,
                  authenticator
                ].sort(
                  (left, right) =>
                    left.sort_order - right.sort_order ||
                    left.name.localeCompare(right.name)
                )
              }
            : overview
      );
      setLifecycleModal(null);
      lifecycleForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    },
    onError: (error) => {
      setOperationErrorMessage(
        authCenterLifecycleErrorMessage(error, 'auto.auth_center_create_failed')
      );
    }
  });
  const copyMutation = useMutation({
    mutationFn: (input: {
      sourceName: string;
      values: AuthenticatorLifecycleFormValues;
    }) =>
      copySettingsAuthCenterAuthenticator(
        input.sourceName,
        {
          name: input.values.name,
          title: input.values.title,
          sort_order: input.values.sort_order ?? undefined
        },
        requireAuthCenterWrite()
      ),
    onSuccess: async (authenticator) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                authenticators: [
                  ...overview.authenticators,
                  authenticator
                ].sort(
                  (left, right) =>
                    left.sort_order - right.sort_order ||
                    left.name.localeCompare(right.name)
                )
              }
            : overview
      );
      setLifecycleModal(null);
      lifecycleForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    },
    onError: (error) => {
      setOperationErrorMessage(
        authCenterLifecycleErrorMessage(error, 'auto.auth_center_copy_failed')
      );
    }
  });
  const deleteMutation = useMutation({
    mutationFn: (authenticatorName: string) =>
      deleteSettingsAuthCenterAuthenticator(
        authenticatorName,
        requireAuthCenterWrite()
      ),
    onSuccess: async (_, authenticatorName) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                authenticators: overview.authenticators.filter(
                  (row) => row.name !== authenticatorName
                )
              }
            : overview
      );
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    },
    onError: (error) => {
      setOperationErrorMessage(
        authCenterLifecycleErrorMessage(error, 'auto.auth_center_delete_failed')
      );
    }
  });
  const reorderMutation = useMutation({
    mutationFn: (names: string[]) =>
      reorderSettingsAuthCenterAuthenticators(names, requireAuthCenterWrite()),
    onSuccess: (overview) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        overview
      );
    },
    onError: (error) => {
      setOperationErrorMessage(
        authCenterLifecycleErrorMessage(
          error,
          'auto.auth_center_reorder_failed'
        )
      );
    }
  });
  const moveAuthenticator = (row: AuthenticatorRow, direction: -1 | 1) => {
    const authenticators = overviewQuery.data?.authenticators ?? [];
    const currentIndex = authenticators.findIndex(
      (authenticator) => authenticator.name === row.name
    );
    const targetIndex = currentIndex + direction;
    if (
      currentIndex < 0 ||
      targetIndex < 0 ||
      targetIndex >= authenticators.length
    ) {
      return;
    }
    const nextRows = [...authenticators];
    const current = nextRows[currentIndex];
    nextRows[currentIndex] = nextRows[targetIndex];
    nextRows[targetIndex] = current;
    setOperationErrorMessage(null);
    reorderMutation.mutate(nextRows.map((authenticator) => authenticator.name));
  };
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
      title: i18nText('settings', 'auto.description'),
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
      render: (_, row, index) => (
        <Space size="small">
          <Tooltip title={i18nText('settings', 'auto.move_up')}>
            <Button
              aria-label={i18nText('settings', 'auto.move_up')}
              icon={<ArrowUpOutlined aria-hidden="true" />}
              size="small"
              type="text"
              disabled={
                index === 0 ||
                !csrfToken ||
                !canManageAuthenticators ||
                reorderMutation.isPending
              }
              onClick={() => moveAuthenticator(row, -1)}
            />
          </Tooltip>
          <Tooltip title={i18nText('settings', 'auto.move_down')}>
            <Button
              aria-label={i18nText('settings', 'auto.move_down')}
              icon={<ArrowDownOutlined aria-hidden="true" />}
              size="small"
              type="text"
              disabled={
                index >= (overviewQuery.data?.authenticators.length ?? 0) - 1 ||
                !csrfToken ||
                !canManageAuthenticators ||
                reorderMutation.isPending
              }
              onClick={() => moveAuthenticator(row, 1)}
            />
          </Tooltip>
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
          <Tooltip title={i18nText('settings', 'auto.copy')}>
            <Button
              aria-label={i18nText('settings', 'auto.copy')}
              icon={<CopyOutlined aria-hidden="true" />}
              size="small"
              type="text"
              disabled={!csrfToken || !canManageAuthenticators}
              onClick={() => openCopyAuthenticator(row)}
            />
          </Tooltip>
          <Popconfirm
            title={i18nText('settings', 'auto.auth_center_delete_confirm', {
              value1: row.title
            })}
            okText={i18nText('settings', 'auto.delete')}
            cancelText={i18nText('settings', 'auto.cancel')}
            disabled={row.is_builtin || !csrfToken || !canManageAuthenticators}
            onConfirm={() => {
              setOperationErrorMessage(null);
              deleteMutation.mutate(row.name);
            }}
          >
            <Tooltip
              title={
                row.is_builtin
                  ? i18nText(
                      'settings',
                      'auto.auth_center_delete_builtin_failed'
                    )
                  : i18nText('settings', 'auto.delete')
              }
            >
              <Button
                aria-label={i18nText('settings', 'auto.delete')}
                danger
                icon={<DeleteOutlined aria-hidden="true" />}
                size="small"
                type="text"
                disabled={
                  row.is_builtin ||
                  !csrfToken ||
                  !canManageAuthenticators ||
                  (deleteMutation.isPending &&
                    deleteMutation.variables === row.name)
                }
              />
            </Tooltip>
          </Popconfirm>
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
          <Flex justify="space-between" align="center" gap="middle">
            <div />
            <Button
              type="primary"
              icon={<PlusOutlined aria-hidden="true" />}
              disabled={
                !csrfToken ||
                !canManageAuthenticators ||
                overviewQuery.data.supported_auth_types.length === 0
              }
              onClick={openCreateAuthenticator}
            >
              {i18nText('settings', 'auto.new')}
            </Button>
          </Flex>
          {operationErrorMessage ? (
            <Alert type="error" message={operationErrorMessage} showIcon />
          ) : null}
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
      <Modal
        open={lifecycleModal != null}
        title={i18nText(
          'settings',
          lifecycleModal?.mode === 'copy'
            ? 'auto.auth_center_copy_authenticator'
            : 'auto.auth_center_create_authenticator'
        )}
        okText={i18nText('settings', 'auto.save')}
        cancelText={i18nText('settings', 'auto.cancel')}
        confirmLoading={createMutation.isPending || copyMutation.isPending}
        onCancel={() => {
          setLifecycleModal(null);
          lifecycleForm.resetFields();
          setOperationErrorMessage(null);
        }}
        onOk={() => {
          lifecycleForm.submit();
        }}
      >
        <Form
          form={lifecycleForm}
          layout="vertical"
          onFinish={(values) => {
            setOperationErrorMessage(null);
            if (lifecycleModal?.mode === 'copy' && lifecycleModal.source) {
              copyMutation.mutate({
                sourceName: lifecycleModal.source.name,
                values
              });
              return;
            }
            createMutation.mutate(values);
          }}
        >
          <Form.Item
            label={i18nText('settings', 'auto.identifier')}
            name="name"
            rules={[
              {
                required: true,
                message: i18nText('settings', 'auto.fill_name')
              },
              {
                pattern: /^[A-Za-z0-9_]+$/,
                message: i18nText(
                  'settings',
                  'auto.auth_center_identifier_invalid'
                )
              }
            ]}
          >
            <Input />
          </Form.Item>
          {lifecycleModal?.mode === 'create' ? (
            <Form.Item
              label={i18nText('settings', 'auto.auth_center_auth_type')}
              name="auth_type"
              rules={[
                {
                  required: true,
                  message: i18nText(
                    'settings',
                    'auto.auth_center_auth_type_required'
                  )
                }
              ]}
            >
              <Select
                options={(overviewQuery.data?.supported_auth_types ?? []).map(
                  (authType) => ({
                    label: authType,
                    value: authType
                  })
                )}
              />
            </Form.Item>
          ) : null}
          <Form.Item
            label={i18nText('settings', 'auto.name')}
            name="title"
            rules={[
              {
                required: true,
                message: i18nText('settings', 'auto.fill_name')
              }
            ]}
          >
            <Input />
          </Form.Item>
          {lifecycleModal?.mode === 'create' ? (
            <>
              <Form.Item
                label={i18nText('settings', 'auto.description')}
                name="description"
              >
                <Input.TextArea autoSize={{ minRows: 3, maxRows: 6 }} />
              </Form.Item>
              <Form.Item
                label={i18nText('settings', 'auto.enabled')}
                name="enabled"
                valuePropName="checked"
              >
                <Switch />
              </Form.Item>
            </>
          ) : null}
          <Form.Item
            label={i18nText('settings', 'auto.auth_center_sort_order')}
            name="sort_order"
          >
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
        </Form>
      </Modal>
    </SettingsSectionSurface>
  );
}
