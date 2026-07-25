import {
  BarsOutlined,
  DeleteOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { useMemo, useState, type DragEvent } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tooltip
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  SchemaFormDrawer,
  type SchemaFormValues
} from '../../../../shared/schema-ui/v1/form-drawer/SchemaFormDrawer';
import type {
  PluginFormSchema,
  PluginFormValue
} from '../../../../shared/schema-ui/v1/contracts/plugin-form-schema';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import {
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
import { AuthenticatorUiBlockStudio } from '../../components/auth-center/AuthenticatorUiBlockStudio';

import './auth-center-panel.css';

type AuthenticatorRow = SettingsAuthCenterOverview['authenticators'][number];

const DEFAULT_AUTH_CENTER_DRAWER_WIDTH = 520;
const MIN_AUTH_CENTER_DRAWER_WIDTH = 480;
const MAX_AUTH_CENTER_DRAWER_WIDTH = 960;
const AUTH_CENTER_DRAG_DATA_TYPE =
  'application/x-1flowbase-auth-center-authenticator-id';

type AuthenticatorConfigFormValues = {
  title: string;
  enabled: boolean;
  description: string | null;
  self_registration_enabled: boolean;
  public_ui_block: string;
  extension_config: Record<string, unknown>;
};

type AuthenticatorLifecycleFormValues = {
  auth_type?: string;
  title: string;
  description?: string | null;
  enabled?: boolean;
};

function authCenterConfigFormValues(row: AuthenticatorRow): SchemaFormValues {
  const description =
    typeof row.config_values.description === 'string'
      ? row.config_values.description
      : null;

  const values: SchemaFormValues = {
    title: row.title,
    enabled: row.enabled,
    description,
    self_registration_enabled:
      row.config_values.self_registration_enabled === true
  };
  for (const field of row.config_schema) {
    const value = row.config_values[field.key];
    if (isPluginFormValue(value)) values[field.key] = value;
  }
  return values;
}

function authCenterConfigFormSchema(row: AuthenticatorRow): PluginFormSchema {
  return {
    schema_version: '1.0.0',
    fields: row.config_schema.map((field) => ({ ...field }))
  };
}

function toAuthenticatorConfigFormValues(
  values: SchemaFormValues,
  row: AuthenticatorRow
): AuthenticatorConfigFormValues {
  const commonKeys = new Set([
    'title', 'description', 'enabled', 'self_registration_enabled'
  ]);
  const extension_config = Object.fromEntries(
    row.config_schema
      .filter((field) => !commonKeys.has(field.key) && values[field.key] !== undefined)
      .map((field) => [field.key, values[field.key]])
  );
  return {
    title: String(values.title ?? ''),
    enabled: Boolean(values.enabled),
    description:
      typeof values.description === 'string' ? values.description : null,
    self_registration_enabled: values.self_registration_enabled === true,
    public_ui_block:
      typeof row.config_values.public_ui_block === 'string'
        ? row.config_values.public_ui_block
        : '',
    extension_config
  };
}

function isPluginFormValue(value: unknown): value is PluginFormValue {
  if (value === null || ['string', 'number', 'boolean'].includes(typeof value)) return true;
  if (Array.isArray(value)) return value.every(isPluginFormValue);
  return typeof value === 'object' && value !== null &&
    Object.values(value).every(isPluginFormValue);
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
    authenticatorId: string,
    values: AuthenticatorConfigFormValues
  ) => Promise<void>;
}) {
  const accessErrorMessage = !canManageAuthenticators
    ? i18nText('settings', 'auto.auth_center_manage_permission_required')
    : !hasCsrfToken
      ? i18nText('settings', 'auto.auth_center_csrf_required')
      : null;
  const initialValues = useMemo(
    () => (authenticator ? authCenterConfigFormValues(authenticator) : {}),
    [authenticator]
  );
  const schema = useMemo(
    () => authenticator
      ? authCenterConfigFormSchema(authenticator)
      : { schema_version: '1.0.0', fields: [] },
    [authenticator]
  );

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
      resizable
      defaultWidth={DEFAULT_AUTH_CENTER_DRAWER_WIDTH}
      minWidth={MIN_AUTH_CENTER_DRAWER_WIDTH}
      maxWidth={MAX_AUTH_CENTER_DRAWER_WIDTH}
      resizeLabel="调整认证器配置抽屉宽度"
      onCancel={onClose}
      onSubmit={(values) =>
        onSubmit(authenticator.id, toAuthenticatorConfigFormValues(values, authenticator))
      }
    />
  ) : null;
}

export function SettingsAuthCenterSection() {
  const [selectedAuthenticatorId, setSelectedAuthenticatorId] = useState<
    string | null
  >(null);
  const [selectedUiAuthenticatorId, setSelectedUiAuthenticatorId] = useState<
    string | null
  >(null);
  const [lifecycleForm] = Form.useForm<AuthenticatorLifecycleFormValues>();
  const [isLifecycleModalOpen, setLifecycleModalOpen] = useState(false);
  const [draggedAuthenticatorId, setDraggedAuthenticatorId] = useState<
    string | null
  >(null);
  const [dragOverAuthenticatorId, setDragOverAuthenticatorId] = useState<
    string | null
  >(null);
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
    selectedAuthenticatorId && overviewQuery.data
      ? (overviewQuery.data.authenticators.find(
          (row) => row.id === selectedAuthenticatorId
        ) ?? null)
      : null;
  const selectedUiAuthenticator =
    selectedUiAuthenticatorId && overviewQuery.data
      ? (overviewQuery.data.authenticators.find(
          (row) => row.id === selectedUiAuthenticatorId
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
      auth_type: overviewQuery.data?.supported_auth_types[0],
      title: '',
      description: null,
      enabled: false
    });
    setLifecycleModalOpen(true);
  };
  const enableMutation = useMutation({
    mutationFn: (authenticatorId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return enableSettingsAuthCenterAuthenticator(
        authenticatorId,
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
      authenticatorId: string;
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
        input.authenticatorId,
        {
          title: input.values.title,
          enabled: input.values.enabled,
          description: input.values.description,
          self_registration_enabled: input.values.self_registration_enabled,
          public_ui_block: input.values.public_ui_block,
          extension_config: input.values.extension_config
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
                  row.id === authenticator.id ? authenticator : row
                )
              }
            : overview
      );
      setSelectedAuthenticatorId(null);
      setSelectedUiAuthenticatorId(null);
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    }
  });
  const createMutation = useMutation({
    mutationFn: (values: AuthenticatorLifecycleFormValues) =>
      createSettingsAuthCenterAuthenticator(
        {
          auth_type: values.auth_type ?? '',
          title: values.title,
          description: values.description ?? null,
          enabled: Boolean(values.enabled),
          sort_order: nextSortOrder
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
                    left.id.localeCompare(right.id)
                )
              }
            : overview
      );
      setLifecycleModalOpen(false);
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
  const deleteMutation = useMutation({
    mutationFn: (authenticatorId: string) =>
      deleteSettingsAuthCenterAuthenticator(
        authenticatorId,
        requireAuthCenterWrite()
      ),
    onSuccess: async (_, authenticatorId) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                authenticators: overview.authenticators.filter(
                  (row) => row.id !== authenticatorId
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
    mutationFn: (ids: string[]) =>
      reorderSettingsAuthCenterAuthenticators(ids, requireAuthCenterWrite()),
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
  const canDragSortAuthenticators =
    Boolean(csrfToken) &&
    canManageAuthenticators &&
    !reorderMutation.isPending &&
    (overviewQuery.data?.authenticators.length ?? 0) > 1;
  const getDraggedAuthenticatorId = (event: DragEvent<HTMLElement>) =>
    draggedAuthenticatorId ||
    event.dataTransfer.getData(AUTH_CENTER_DRAG_DATA_TYPE);
  const dropAuthenticatorBeforeTarget = (
    sourceAuthenticatorId: string,
    targetAuthenticatorId: string
  ) => {
    const authenticators = overviewQuery.data?.authenticators ?? [];
    const sourceIndex = authenticators.findIndex(
      (authenticator) => authenticator.id === sourceAuthenticatorId
    );
    const targetIndex = authenticators.findIndex(
      (authenticator) => authenticator.id === targetAuthenticatorId
    );
    if (
      sourceIndex < 0 ||
      targetIndex < 0 ||
      sourceIndex === targetIndex ||
      !canDragSortAuthenticators
    ) {
      return;
    }

    const nextRows = [...authenticators];
    const [movedAuthenticator] = nextRows.splice(sourceIndex, 1);
    nextRows.splice(targetIndex, 0, movedAuthenticator);
    setOperationErrorMessage(null);
    reorderMutation.mutate(nextRows.map((authenticator) => authenticator.id));
  };
  const authenticatorColumns: ColumnsType<AuthenticatorRow> = [
    {
      title: i18nText('settings', 'auto.auth_center_sequence'),
      key: 'sequence',
      width: 112,
      render: (_, row, index) => (
        <Space size={6} className="settings-auth-center__sequence">
          <Tooltip
            title={i18nText(
              'settings',
              'auto.auth_center_drag_sort_authenticator',
              { value1: row.title }
            )}
          >
            <Button
              aria-label={i18nText(
                'settings',
                'auto.auth_center_drag_sort_authenticator',
                { value1: row.title }
              )}
              className="settings-auth-center__drag-handle"
              disabled={!canDragSortAuthenticators}
              draggable={canDragSortAuthenticators}
              icon={<BarsOutlined aria-hidden="true" />}
              size="small"
              type="text"
              onClick={(event) => {
                event.stopPropagation();
              }}
              onDragEnd={(event) => {
                event.stopPropagation();
                setDraggedAuthenticatorId(null);
                setDragOverAuthenticatorId(null);
              }}
              onDragStart={(event) => {
                event.stopPropagation();
                event.dataTransfer.effectAllowed = 'move';
                event.dataTransfer.setData(
                  AUTH_CENTER_DRAG_DATA_TYPE,
                  row.id
                );
                setDraggedAuthenticatorId(row.id);
              }}
            />
          </Tooltip>
          <span>{index + 1}</span>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.name'),
      dataIndex: 'title',
      key: 'title'
    },
    {
      title: i18nText('settings', 'auto.auth_center_category'),
      dataIndex: 'auth_type',
      key: 'auth_type'
    },
    {
      title: i18nText('settings', 'auto.description'),
      key: 'description',
      render: (_, row) => row.config_values.description || '-'
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
            enableMutation.isPending && enableMutation.variables === row.id
          }
          onChange={(checked) => {
            if (checked && !enabled) {
              enableMutation.mutate(row.id);
            }
          }}
        />
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'operation',
      align: 'right',
      className: 'settings-auth-center__operation-cell',
      width: 1,
      render: (_, row) => (
        <Space size="small" wrap={false}>
          <Button
            type="link"
            size="small"
            onClick={() => {
              configMutation.reset();
              setSelectedAuthenticatorId(row.id);
            }}
          >
            {i18nText('settings', 'auto.edit')}
          </Button>
          <Button
            type="link"
            size="small"
            onClick={() => {
              configMutation.reset();
              setSelectedUiAuthenticatorId(row.id);
            }}
          >
            {i18nText('settings', 'auto.auth_center_ui_action')}
          </Button>
          <Popconfirm
            title={i18nText('settings', 'auto.auth_center_delete_confirm', {
              value1: row.title
            })}
            okText={i18nText('settings', 'auto.delete')}
            cancelText={i18nText('settings', 'auto.cancel')}
            disabled={row.is_builtin || !csrfToken || !canManageAuthenticators}
            onConfirm={() => {
              setOperationErrorMessage(null);
              deleteMutation.mutate(row.id);
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
                    deleteMutation.variables === row.id)
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
            className="settings-auth-center__table"
            rowKey="id"
            columns={authenticatorColumns}
            dataSource={overviewQuery.data.authenticators}
            onRow={(row) => ({
              className:
                dragOverAuthenticatorId === row.id
                  ? 'settings-auth-center__row--drag-over'
                  : undefined,
              onDragLeave: () => {
                setDragOverAuthenticatorId((currentId) =>
                  currentId === row.id ? null : currentId
                );
              },
              onDragOver: (event) => {
                const sourceAuthenticatorId = getDraggedAuthenticatorId(event);
                if (
                  !canDragSortAuthenticators ||
                  !sourceAuthenticatorId ||
                  sourceAuthenticatorId === row.id
                ) {
                  return;
                }

                event.preventDefault();
                event.dataTransfer.dropEffect = 'move';
                setDragOverAuthenticatorId(row.id);
              },
              onDrop: (event) => {
                event.preventDefault();
                const sourceAuthenticatorId = getDraggedAuthenticatorId(event);
                setDraggedAuthenticatorId(null);
                setDragOverAuthenticatorId(null);
                if (!sourceAuthenticatorId || sourceAuthenticatorId === row.id) {
                  return;
                }

                dropAuthenticatorBeforeTarget(sourceAuthenticatorId, row.id);
              }
            })}
            pagination={false}
            size="middle"
          />
        </Flex>
      ) : null}
      <AuthenticatorConfigDrawer
        authenticator={selectedAuthenticator}
        open={selectedAuthenticatorId != null}
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
          setSelectedAuthenticatorId(null);
        }}
        onSubmit={async (authenticatorId, values) => {
          await new Promise<void>((resolve, reject) => {
            configMutation.mutate(
              { authenticatorId, values },
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
      {selectedUiAuthenticator ? (
        <AuthenticatorUiBlockStudio
          authenticatorId={selectedUiAuthenticator.id}
          authenticatorTitle={selectedUiAuthenticator.title}
          authType={selectedUiAuthenticator.auth_type}
          contextVariables={selectedUiAuthenticator.context_variables}
          description={
            typeof selectedUiAuthenticator.config_values.description ===
            'string'
              ? selectedUiAuthenticator.config_values.description
              : null
          }
          enabled={selectedUiAuthenticator.enabled}
          interfacePathPrefixes={
            selectedUiAuthenticator.interface_path_prefixes
          }
          publicVariables={selectedUiAuthenticator.public_variables}
          selfRegistrationEnabled={
            selectedUiAuthenticator.config_values
              .self_registration_enabled === true
          }
          workspaceId={actor?.current_workspace_id ?? ''}
          errorMessage={
            configMutation.isError
              ? configMutation.error instanceof Error &&
                configMutation.error.message.trim().length > 0
                ? configMutation.error.message
                : i18nText(
                    'settings',
                    'auto.auth_center_public_ui_update_failed'
                  )
              : null
          }
          open={selectedUiAuthenticatorId != null}
          readOnly={!canManageAuthenticators || csrfToken == null}
          saving={configMutation.isPending}
          source={
            typeof selectedUiAuthenticator.config_values.public_ui_block ===
            'string'
              ? selectedUiAuthenticator.config_values.public_ui_block
              : ''
          }
          onClose={() => {
            configMutation.reset();
            setSelectedUiAuthenticatorId(null);
          }}
          onSave={async (publicUiBlock) => {
            const extensionConfig =
              selectedUiAuthenticator.config_values.extension_config;
            await new Promise<void>((resolve, reject) => {
              configMutation.mutate(
                {
                  authenticatorId: selectedUiAuthenticator.id,
                  values: {
                    title: selectedUiAuthenticator.title,
                    enabled: selectedUiAuthenticator.enabled,
                    description:
                      typeof selectedUiAuthenticator.config_values
                        .description === 'string'
                        ? selectedUiAuthenticator.config_values.description
                        : null,
                    self_registration_enabled:
                      selectedUiAuthenticator.config_values
                        .self_registration_enabled === true,
                    public_ui_block: publicUiBlock,
                    extension_config:
                      extensionConfig &&
                      typeof extensionConfig === 'object' &&
                      !Array.isArray(extensionConfig)
                        ? extensionConfig as Record<string, unknown>
                        : {}
                  }
                },
                {
                  onError: (error) => reject(error),
                  onSuccess: () => resolve()
                }
              );
            });
          }}
        />
      ) : null}
      <Modal
        open={isLifecycleModalOpen}
        title={i18nText(
          'settings',
          'auto.auth_center_create_authenticator'
        )}
        okText={i18nText('settings', 'auto.save')}
        cancelText={i18nText('settings', 'auto.cancel')}
        confirmLoading={createMutation.isPending}
        onCancel={() => {
          setLifecycleModalOpen(false);
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
            createMutation.mutate(values);
          }}
        >
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
        </Form>
      </Modal>
    </SettingsSectionSurface>
  );
}
