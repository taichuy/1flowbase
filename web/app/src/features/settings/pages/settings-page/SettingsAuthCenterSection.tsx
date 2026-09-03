import BarsOutlined from '@ant-design/icons/es/icons/BarsOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
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
  createSettingsAuthCenterLoginEntry,
  deleteSettingsAuthCenterLoginEntry,
  fetchSettingsAuthCenterOverview,
  reorderSettingsAuthCenterLoginEntries,
  settingsAuthCenterOverviewQueryKey,
  updateSettingsAuthCenterLoginEntryEnabled,
  updateSettingsAuthCenterLoginEntryConfig,
  updateSettingsAuthCenterLoginEntryPublicUiBlock,
  type SettingsAuthCenterOverview
} from '../../api/auth-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { LoginEntryUiBlockStudio } from '../../components/auth-center/LoginEntryUiBlockStudio';

import './auth-center-panel.css';

type LoginEntryRow = SettingsAuthCenterOverview['login_entries'][number];

const DEFAULT_AUTH_CENTER_DRAWER_WIDTH = 520;
const MIN_AUTH_CENTER_DRAWER_WIDTH = 480;
const MAX_AUTH_CENTER_DRAWER_WIDTH = 960;
const AUTH_CENTER_DRAG_DATA_TYPE =
  'application/x-1flowbase-auth-center-authenticator-id';

type LoginEntryConfigFormValues = {
  title: string;
  enabled: boolean;
  description: string | null;
  self_registration_enabled: boolean;
  extension_config: Record<string, unknown>;
};

type LoginEntryLifecycleFormValues = {
  auth_type?: string;
  title: string;
  description?: string | null;
  enabled?: boolean;
};

function authCenterConfigFormValues(row: LoginEntryRow): SchemaFormValues {
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

function authCenterConfigFormSchema(row: LoginEntryRow): PluginFormSchema {
  return {
    schema_version: '1.0.0',
    fields: row.config_schema.map((field) => ({ ...field }))
  };
}

function toLoginEntryConfigFormValues(
  values: SchemaFormValues,
  row: LoginEntryRow
): LoginEntryConfigFormValues {
  const commonKeys = new Set([
    'title',
    'description',
    'enabled',
    'self_registration_enabled'
  ]);
  const extension_config = Object.fromEntries(
    row.config_schema
      .filter(
        (field) => !commonKeys.has(field.key) && values[field.key] !== undefined
      )
      .map((field) => [field.key, values[field.key]])
  );
  return {
    title: String(values.title ?? ''),
    enabled: Boolean(values.enabled),
    description:
      typeof values.description === 'string' ? values.description : null,
    self_registration_enabled: values.self_registration_enabled === true,
    extension_config
  };
}

function isPluginFormValue(value: unknown): value is PluginFormValue {
  if (value === null || ['string', 'number', 'boolean'].includes(typeof value))
    return true;
  if (Array.isArray(value)) return value.every(isPluginFormValue);
  return (
    typeof value === 'object' &&
    value !== null &&
    Object.values(value).every(isPluginFormValue)
  );
}

function authCenterLifecycleErrorMessage(error: unknown, fallbackKey: string) {
  const code =
    error && typeof error === 'object' && 'code' in error
      ? (error as { code?: unknown }).code
      : null;
  if (code === 'builtin_login_entry') {
    return i18nText('settings', 'auto.auth_center_delete_builtin_failed');
  }

  return i18nText('settings', fallbackKey);
}

function LoginEntryConfigDrawer({
  authenticator,
  open,
  canManageLoginEntries,
  hasCsrfToken,
  submitting,
  onClose,
  onSubmit
}: {
  authenticator: LoginEntryRow | null;
  open: boolean;
  canManageLoginEntries: boolean;
  hasCsrfToken: boolean;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (
    loginEntryId: string,
    values: LoginEntryConfigFormValues
  ) => Promise<void>;
}) {
  const accessErrorMessage = !canManageLoginEntries
    ? i18nText('settings', 'auto.auth_center_manage_permission_required')
    : !hasCsrfToken
      ? i18nText('settings', 'auto.auth_center_csrf_required')
      : null;
  const initialValues = useMemo(
    () => (authenticator ? authCenterConfigFormValues(authenticator) : {}),
    [authenticator]
  );
  const schema = useMemo(
    () =>
      authenticator
        ? authCenterConfigFormSchema(authenticator)
        : { schema_version: '1.0.0', fields: [] },
    [authenticator]
  );

  const statusMessages = accessErrorMessage
    ? [
        {
          key: 'access-error',
          message: accessErrorMessage,
          type: 'error' as const
        }
      ]
    : [];

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
        onSubmit(
          authenticator.id,
          toLoginEntryConfigFormValues(values, authenticator)
        )
      }
    />
  ) : null;
}

export function SettingsAuthCenterSection() {
  const [selectedLoginEntryId, setSelectedLoginEntryId] = useState<
    string | null
  >(null);
  const [selectedUiLoginEntryId, setSelectedUiLoginEntryId] = useState<
    string | null
  >(null);
  const [lifecycleForm] = Form.useForm<LoginEntryLifecycleFormValues>();
  const [isLifecycleModalOpen, setLifecycleModalOpen] = useState(false);
  const [draggedLoginEntryId, setDraggedLoginEntryId] = useState<
    string | null
  >(null);
  const [dragOverLoginEntryId, setDragOverLoginEntryId] = useState<
    string | null
  >(null);
  const [operationErrorMessage, setOperationErrorMessage] = useState<
    string | null
  >(null);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const queryClient = useQueryClient();
  const canManageLoginEntries =
    actor?.effective_display_role === 'root' ||
    (me?.permissions ?? []).includes('user.manage.all');
  const overviewQuery = useQuery({
    queryKey: settingsAuthCenterOverviewQueryKey,
    queryFn: fetchSettingsAuthCenterOverview
  });
  const selectedLoginEntry =
    selectedLoginEntryId && overviewQuery.data
      ? (overviewQuery.data.login_entries.find(
          (row) => row.id === selectedLoginEntryId
        ) ?? null)
      : null;
  const selectedUiLoginEntry =
    selectedUiLoginEntryId && overviewQuery.data
      ? (overviewQuery.data.login_entries.find(
          (row) => row.id === selectedUiLoginEntryId
        ) ?? null)
      : null;
  const requireAuthCenterWrite = () => {
    if (!canManageLoginEntries) {
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
    (overviewQuery.data?.login_entries.at(-1)?.sort_order ?? 0) + 10;
  const openCreateLoginEntry = () => {
    setOperationErrorMessage(null);
    lifecycleForm.setFieldsValue({
      auth_type: overviewQuery.data?.supported_auth_types[0],
      title: '',
      description: null,
      enabled: false
    });
    setLifecycleModalOpen(true);
  };
  const enabledMutation = useMutation({
    mutationFn: (input: { loginEntryId: string; enabled: boolean }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }
      return updateSettingsAuthCenterLoginEntryEnabled(
        input.loginEntryId,
        { enabled: input.enabled },
        csrfToken
      );
    },
    onMutate: () => {
      setOperationErrorMessage(null);
    },
    onSuccess: async (authenticator) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                login_entries: overview.login_entries.map((row) =>
                  row.id === authenticator.id ? authenticator : row
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
        error instanceof Error ? error.message : String(error)
      );
    }
  });
  const configMutation = useMutation({
    mutationFn: (input: {
      loginEntryId: string;
      values: LoginEntryConfigFormValues;
    }) => {
      if (!canManageLoginEntries) {
        throw new Error(
          i18nText('settings', 'auto.auth_center_manage_permission_required')
        );
      }
      if (!csrfToken) {
        throw new Error(i18nText('settings', 'auto.auth_center_csrf_required'));
      }
      return updateSettingsAuthCenterLoginEntryConfig(
        input.loginEntryId,
        {
          title: input.values.title,
          enabled: input.values.enabled,
          description: input.values.description,
          self_registration_enabled: input.values.self_registration_enabled,
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
                login_entries: overview.login_entries.map((row) =>
                  row.id === authenticator.id ? authenticator : row
                )
              }
            : overview
      );
      setSelectedLoginEntryId(null);
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    }
  });
  const publicUiBlockMutation = useMutation({
    mutationFn: (input: { loginEntryId: string; publicUiBlock: string }) => {
      if (!canManageLoginEntries) {
        throw new Error(
          i18nText('settings', 'auto.auth_center_manage_permission_required')
        );
      }
      if (!csrfToken) {
        throw new Error(i18nText('settings', 'auto.auth_center_csrf_required'));
      }
      return updateSettingsAuthCenterLoginEntryPublicUiBlock(
        input.loginEntryId,
        { public_ui_block: input.publicUiBlock },
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
                login_entries: overview.login_entries.map((row) =>
                  row.id === authenticator.id ? authenticator : row
                )
              }
            : overview
      );
      await queryClient.invalidateQueries({
        queryKey: settingsAuthCenterOverviewQueryKey
      });
    }
  });
  const createMutation = useMutation({
    mutationFn: (values: LoginEntryLifecycleFormValues) =>
      createSettingsAuthCenterLoginEntry(
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
                login_entries: [
                  ...overview.login_entries,
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
    mutationFn: (loginEntryId: string) =>
      deleteSettingsAuthCenterLoginEntry(
        loginEntryId,
        requireAuthCenterWrite()
      ),
    onSuccess: async (_, loginEntryId) => {
      queryClient.setQueryData<SettingsAuthCenterOverview>(
        settingsAuthCenterOverviewQueryKey,
        (overview) =>
          overview
            ? {
                ...overview,
                login_entries: overview.login_entries.filter(
                  (row) => row.id !== loginEntryId
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
      reorderSettingsAuthCenterLoginEntries(ids, requireAuthCenterWrite()),
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
  const canDragSortLoginEntries =
    Boolean(csrfToken) &&
    canManageLoginEntries &&
    !reorderMutation.isPending &&
    (overviewQuery.data?.login_entries.length ?? 0) > 1;
  const getDraggedLoginEntryId = (event: DragEvent<HTMLElement>) =>
    draggedLoginEntryId ||
    event.dataTransfer.getData(AUTH_CENTER_DRAG_DATA_TYPE);
  const dropLoginEntryBeforeTarget = (
    sourceLoginEntryId: string,
    targetLoginEntryId: string
  ) => {
    const login_entries = overviewQuery.data?.login_entries ?? [];
    const sourceIndex = login_entries.findIndex(
      (authenticator) => authenticator.id === sourceLoginEntryId
    );
    const targetIndex = login_entries.findIndex(
      (authenticator) => authenticator.id === targetLoginEntryId
    );
    if (
      sourceIndex < 0 ||
      targetIndex < 0 ||
      sourceIndex === targetIndex ||
      !canDragSortLoginEntries
    ) {
      return;
    }

    const nextRows = [...login_entries];
    const [movedLoginEntry] = nextRows.splice(sourceIndex, 1);
    nextRows.splice(targetIndex, 0, movedLoginEntry);
    setOperationErrorMessage(null);
    reorderMutation.mutate(nextRows.map((authenticator) => authenticator.id));
  };
  const authenticatorColumns: ColumnsType<LoginEntryRow> = [
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
              disabled={!canDragSortLoginEntries}
              draggable={canDragSortLoginEntries}
              icon={<BarsOutlined aria-hidden="true" />}
              size="small"
              type="text"
              onClick={(event) => {
                event.stopPropagation();
              }}
              onDragEnd={(event) => {
                event.stopPropagation();
                setDraggedLoginEntryId(null);
                setDragOverLoginEntryId(null);
              }}
              onDragStart={(event) => {
                event.stopPropagation();
                event.dataTransfer.effectAllowed = 'move';
                event.dataTransfer.setData(AUTH_CENTER_DRAG_DATA_TYPE, row.id);
                setDraggedLoginEntryId(row.id);
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
          disabled={!csrfToken || !canManageLoginEntries}
          loading={
            enabledMutation.isPending &&
            enabledMutation.variables?.loginEntryId === row.id
          }
          onChange={(checked) => {
            enabledMutation.mutate({
              loginEntryId: row.id,
              enabled: checked
            });
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
              setSelectedLoginEntryId(row.id);
            }}
          >
            {i18nText('settings', 'auto.edit')}
          </Button>
          <Button
            type="link"
            size="small"
            onClick={() => {
              publicUiBlockMutation.reset();
              setSelectedUiLoginEntryId(row.id);
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
            disabled={row.is_builtin || !csrfToken || !canManageLoginEntries}
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
                  !canManageLoginEntries ||
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
    <SettingsSectionSurface heightMode="fill">
      {overviewQuery.isLoading ? <LoadingState compact /> : null}
      {overviewQuery.isError ? (
        <Alert
          type="error"
          title={i18nText('settings', 'auto.auth_center_overview_load_failed')}
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
                !canManageLoginEntries ||
                overviewQuery.data.supported_auth_types.length === 0
              }
              onClick={openCreateLoginEntry}
            >
              {i18nText('settings', 'auto.new')}
            </Button>
          </Flex>
          {operationErrorMessage ? (
            <Alert type="error" title={operationErrorMessage} showIcon />
          ) : null}
          <Table
            className="settings-auth-center__table"
            rowKey="id"
            columns={authenticatorColumns}
            dataSource={overviewQuery.data.login_entries}
            onRow={(row) => ({
              className:
                dragOverLoginEntryId === row.id
                  ? 'settings-auth-center__row--drag-over'
                  : undefined,
              onDragLeave: () => {
                setDragOverLoginEntryId((currentId) =>
                  currentId === row.id ? null : currentId
                );
              },
              onDragOver: (event) => {
                const sourceLoginEntryId = getDraggedLoginEntryId(event);
                if (
                  !canDragSortLoginEntries ||
                  !sourceLoginEntryId ||
                  sourceLoginEntryId === row.id
                ) {
                  return;
                }

                event.preventDefault();
                event.dataTransfer.dropEffect = 'move';
                setDragOverLoginEntryId(row.id);
              },
              onDrop: (event) => {
                event.preventDefault();
                const sourceLoginEntryId = getDraggedLoginEntryId(event);
                setDraggedLoginEntryId(null);
                setDragOverLoginEntryId(null);
                if (
                  !sourceLoginEntryId ||
                  sourceLoginEntryId === row.id
                ) {
                  return;
                }

                dropLoginEntryBeforeTarget(sourceLoginEntryId, row.id);
              }
            })}
            pagination={false}
            size="middle"
          />
        </Flex>
      ) : null}
      <LoginEntryConfigDrawer
        authenticator={selectedLoginEntry}
        open={selectedLoginEntryId != null}
        canManageLoginEntries={canManageLoginEntries}
        hasCsrfToken={csrfToken != null}
        submitting={configMutation.isPending}
        onClose={() => {
          configMutation.reset();
          setSelectedLoginEntryId(null);
        }}
        onSubmit={async (loginEntryId, values) => {
          await new Promise<void>((resolve, reject) => {
            configMutation.mutate(
              { loginEntryId, values },
              {
                onError: (error) => reject(error),
                onSuccess: () => resolve()
              }
            );
          });
        }}
      />
      {selectedUiLoginEntry ? (
        <LoginEntryUiBlockStudio
          loginEntryId={selectedUiLoginEntry.id}
          loginEntryTitle={selectedUiLoginEntry.title}
          authType={selectedUiLoginEntry.auth_type}
          contextVariables={selectedUiLoginEntry.context_variables}
          defaultSource={
            selectedUiLoginEntry.default_public_ui_block ?? null
          }
          description={
            typeof selectedUiLoginEntry.config_values.description ===
            'string'
              ? selectedUiLoginEntry.config_values.description
              : null
          }
          enabled={selectedUiLoginEntry.enabled}
          interfacePathPrefixes={
            selectedUiLoginEntry.interface_path_prefixes
          }
          publicVariables={selectedUiLoginEntry.public_variables}
          selfRegistrationEnabled={
            selectedUiLoginEntry.config_values.self_registration_enabled ===
            true
          }
          workspaceId={actor?.current_workspace_id ?? ''}
          errorMessage={
            publicUiBlockMutation.isError
              ? publicUiBlockMutation.error instanceof Error &&
                publicUiBlockMutation.error.message.trim().length > 0
                ? publicUiBlockMutation.error.message
                : i18nText(
                    'settings',
                    'auto.auth_center_public_ui_update_failed'
                  )
              : null
          }
          open={selectedUiLoginEntryId != null}
          readOnly={!canManageLoginEntries || csrfToken == null}
          saving={publicUiBlockMutation.isPending}
          source={selectedUiLoginEntry.public_ui_block}
          onClose={() => {
            publicUiBlockMutation.reset();
            setSelectedUiLoginEntryId(null);
          }}
          onSave={async (publicUiBlock) => {
            await new Promise<void>((resolve, reject) => {
              publicUiBlockMutation.mutate(
                {
                  loginEntryId: selectedUiLoginEntry.id,
                  publicUiBlock
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
        title={i18nText('settings', 'auto.auth_center_create_authenticator')}
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
