import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  LeftOutlined,
  PlusOutlined,
  ReloadOutlined,
  RightOutlined,
  SaveOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Divider,
  Flex,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Segmented,
  Space,
  Steps,
  Switch,
  Table,
  Tabs,
  Tag,
  Tooltip,
  Tree,
  Typography,
  message
} from 'antd';
import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type SetStateAction
} from 'react';
import type { ColumnsType } from 'antd/es/table';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpInterfaceCapability,
  ConsoleMcpMetaToolConfig,
  ConsoleMcpTool,
  ConsoleMcpToolBinding,
  SaveConsoleMcpInstanceBody,
  SaveConsoleMcpToolBody
} from '@1flowbase/api-client';

import {
  createSettingsMcpInstance,
  createSettingsMcpTool,
  createSettingsMcpToolBinding,
  deleteSettingsMcpGroup,
  deleteSettingsMcpInstance,
  deleteSettingsMcpTool,
  deleteSettingsMcpToolBinding,
  exportSettingsMcpCatalog,
  exportSettingsMcpInstanceDirectory,
  refreshSettingsMcpToolDescription,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpInstance,
  updateSettingsMcpMetaToolConfig,
  updateSettingsMcpTool,
  updateSettingsMcpToolBinding,
  upsertSettingsMcpGroup
} from '../../api/mcp-management';
import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import { i18nText } from '../../../../shared/i18n/text';
import {
  buildMcpDirectoryTreeData,
  buildRandomToolIdSeed,
  buildReadableToolId
} from './mcp-management-view-model';
import {
  buildInputMappingFromInterface,
  inputMappingHasContent,
  normalizeInputMapping,
  type McpInputMappingValue
} from './mcp-input-mapping-model';
import { McpInputMappingEditor } from './McpInputMappingEditor';
import { JsonSchemaInlineEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel';
import {
  createInitialMcpInstancesState,
  initialMcpToolsState,
  mcpInstancesReducer,
  mcpToolsReducer
} from './mcp-management-state';
import {
  downloadMcpExportPackage,
  parseJsonText,
  riskColor,
  statusColor,
  stringifyJson
} from './mcp-management-utils';
import './mcp-management-panel.css';

type InstanceFormValues = SaveConsoleMcpInstanceBody;
type GroupFormValues = {
  instance_id: string;
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
};
type BindingFormValues = {
  instance_id: string;
  group_path: string;
  tool_id: string;
  display_alias: string | null;
  visible: boolean;
  sort_order: number;
};

type ToolFormValues = {
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  usage_description: string | null;
  full_description: string;
  interface_id: string;
  input_mapping: McpInputMappingValue;
  output_mapping: Record<string, unknown>;
  audit_policy_text: string;
  status: string;
};
const TOOL_FORM_STEPS = [
  { title: 'basic', label: 'basic', value: 'basic' },
  { title: 'interface', label: 'interface', value: 'interface' },
  { title: 'input', label: 'input_mapping', value: 'input' },
  { title: 'output', label: 'output_mapping', value: 'output' },
  { title: 'description', label: 'preview', value: 'description' }
];
type MetaToolConfigFormValues = Omit<
  ConsoleMcpMetaToolConfig,
  'list_return_fields'
> & {
  list_return_fields_text: string;
};

function useCsrfToken() {
  return useAuthStore((state) => state.csrfToken ?? '');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function emptyObjectSchema(): Record<string, unknown> {
  return {
    type: 'object',
    properties: {},
    additionalProperties: false
  };
}

function schemaRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : emptyObjectSchema();
}

function interfaceOptionLabel(entry: ConsoleMcpInterfaceCapability) {
  return `${entry.method} ${entry.path}`;
}

function SelectedInterfaceOperationTitle({
  selectedInterface
}: {
  selectedInterface: ConsoleMcpInterfaceCapability | undefined;
}) {
  if (!selectedInterface) {
    return null;
  }

  return (
    <Typography.Text>{interfaceOptionLabel(selectedInterface)}</Typography.Text>
  );
}

function schemaMappingHasContent(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }

  const properties = value.properties;
  if (isRecord(properties) && Object.keys(properties).length > 0) {
    return true;
  }

  if (Array.isArray(value.required) && value.required.length > 0) {
    return true;
  }

  if (isRecord(value.items) && schemaMappingHasContent(value.items)) {
    return true;
  }

  return Object.entries(value).some(([key, entry]) => {
    if (key === 'type' && (entry === 'object' || entry === 'array')) {
      return false;
    }
    if (
      key === 'properties' &&
      isRecord(entry) &&
      Object.keys(entry).length === 0
    ) {
      return false;
    }
    if (key === 'additionalProperties' && entry === false) {
      return false;
    }
    if (Array.isArray(entry) && entry.length === 0) {
      return false;
    }

    return entry !== undefined;
  });
}

export function McpManagementPanel({
  canManage,
  catalog,
  interfaceCapabilities
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
  interfaceCapabilities: ConsoleMcpInterfaceCapability[];
}) {
  return (
    <Tabs
      className="mcp-management"
      items={[
        {
          key: 'instances',
          label: i18nText('settings', 'auto.mcp_instances'),
          children: <McpInstancesTab canManage={canManage} catalog={catalog} />
        },
        {
          key: 'tools',
          label: i18nText('settings', 'auto.mcp_tool_config'),
          children: (
            <McpToolsTab
              canManage={canManage}
              catalog={catalog}
              interfaceCapabilities={interfaceCapabilities}
            />
          )
        },
        {
          key: 'meta',
          label: i18nText('settings', 'auto.mcp_meta_config'),
          children: (
            <McpMetaConfigTab
              canManage={canManage}
              metaToolConfig={catalog.meta_tool_config}
            />
          )
        }
      ]}
    />
  );
}

function McpInstancesTab({
  canManage,
  catalog
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
}) {
  const csrfToken = useCsrfToken();
  const queryClient = useQueryClient();
  const [instanceForm] = Form.useForm<InstanceFormValues>();
  const [groupForm] = Form.useForm<GroupFormValues>();
  const [bindingForm] = Form.useForm<BindingFormValues>();
  const [instancesState, dispatchInstancesState] = useReducer(
    mcpInstancesReducer,
    catalog.instances[0]?.instance_id ?? '',
    createInitialMcpInstancesState
  );
  const {
    editingInstance,
    editingBinding,
    instanceModalOpen,
    exportingInstances,
    requestedInstanceId
  } = instancesState;
  const setEditingInstance = useCallback(
    (value: SetStateAction<ConsoleMcpInstance | null>) =>
      dispatchInstancesState({ type: 'setEditingInstance', value }),
    []
  );
  const setEditingBinding = useCallback(
    (value: SetStateAction<ConsoleMcpToolBinding | null>) =>
      dispatchInstancesState({ type: 'setEditingBinding', value }),
    []
  );
  const setInstanceModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchInstancesState({ type: 'setInstanceModalOpen', value }),
    []
  );
  const setExportingInstances = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchInstancesState({ type: 'setExportingInstances', value }),
    []
  );
  const setRequestedInstanceId = useCallback(
    (value: SetStateAction<string>) =>
      dispatchInstancesState({ type: 'setRequestedInstanceId', value }),
    []
  );
  const fallbackInstanceId = catalog.instances[0]?.instance_id ?? '';
  const selectedInstanceId = catalog.instances.some(
    (instance) => instance.instance_id === requestedInstanceId
  )
    ? requestedInstanceId
    : fallbackInstanceId;

  const saveInstanceMutation = useMutation({
    mutationFn: (values: InstanceFormValues) => {
      if (editingInstance) {
        return updateSettingsMcpInstance(
          editingInstance.instance_id,
          values,
          csrfToken
        );
      }
      return createSettingsMcpInstance(values, csrfToken);
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      setInstanceModalOpen(false);
      setEditingInstance(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteInstanceMutation = useMutation({
    mutationFn: (instanceId: string) =>
      deleteSettingsMcpInstance(instanceId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const saveGroupMutation = useMutation({
    mutationFn: (values: GroupFormValues) =>
      upsertSettingsMcpGroup(
        values.instance_id,
        {
          path: values.path,
          display_name: values.display_name,
          description_short: values.description_short,
          enabled: values.enabled,
          sort_order: values.sort_order
        },
        csrfToken
      ),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      groupForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteGroupMutation = useMutation({
    mutationFn: (values: { instanceId: string; path: string }) =>
      deleteSettingsMcpGroup(values.instanceId, values.path, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      groupForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const saveBindingMutation = useMutation({
    mutationFn: (values: BindingFormValues) => {
      const body = {
        group_path: values.group_path,
        tool_id: values.tool_id,
        display_alias: values.display_alias,
        visible: values.visible,
        sort_order: values.sort_order
      };

      if (editingBinding) {
        return updateSettingsMcpToolBinding(editingBinding.id, body, csrfToken);
      }

      return createSettingsMcpToolBinding(values.instance_id, body, csrfToken);
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      bindingForm.resetFields();
      setEditingBinding(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteBindingMutation = useMutation({
    mutationFn: (bindingId: string) =>
      deleteSettingsMcpToolBinding(bindingId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  async function handleExportInstances() {
    setExportingInstances(true);
    try {
      const exportPackage = await exportSettingsMcpInstanceDirectory();
      downloadMcpExportPackage(exportPackage);
      message.success(i18nText('settings', 'auto.mcp_export_ready'));
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingInstances(false);
    }
  }

  const groupCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const group of catalog.groups) {
      counts.set(
        group.instance_record_id,
        (counts.get(group.instance_record_id) ?? 0) + 1
      );
    }
    return counts;
  }, [catalog.groups]);
  const toolCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const binding of catalog.bindings) {
      counts.set(
        binding.instance_record_id,
        (counts.get(binding.instance_record_id) ?? 0) + 1
      );
    }
    return counts;
  }, [catalog.bindings]);
  const selectedInstance = useMemo(
    () =>
      catalog.instances.find(
        (instance) => instance.instance_id === selectedInstanceId
      ) ?? catalog.instances[0],
    [catalog.instances, selectedInstanceId]
  );
  const directoryTreeData = useMemo(() => {
    if (!selectedInstance) {
      return [];
    }

    return buildMcpDirectoryTreeData({
      instance: selectedInstance,
      groups: catalog.groups,
      bindings: catalog.bindings,
      tools: catalog.tools
    });
  }, [catalog.bindings, catalog.groups, catalog.tools, selectedInstance]);

  function resolveInstanceId(binding: ConsoleMcpToolBinding) {
    return (
      catalog.instances.find(
        (instance) => instance.id === binding.instance_record_id
      )?.instance_id ??
      selectedInstance?.instance_id ??
      ''
    );
  }

  const instanceColumns: ColumnsType<ConsoleMcpInstance> = [
    {
      title: i18nText('settings', 'auto.instance_name'),
      dataIndex: 'name',
      render: (_, record) => (
        <Space direction="vertical" size={0}>
          <Typography.Text strong>{record.name}</Typography.Text>
          <Typography.Text type="secondary">
            {record.instance_id}
          </Typography.Text>
        </Space>
      )
    },
    {
      title: i18nText('settingsMcpManagement', 'auto.instance_description'),
      dataIndex: 'description_short',
      render: (description: ConsoleMcpInstance['description_short']) => (
        <Typography.Text type={description ? undefined : 'secondary'}>
          {description || '-'}
        </Typography.Text>
      )
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      render: (status: string) => (
        <Tag color={statusColor(status)}>{status}</Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.directory_summary'),
      render: (_, record) => (
        <Typography.Text>
          {groupCounts.get(record.id) ?? 0} / {toolCounts.get(record.id) ?? 0}
        </Typography.Text>
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      render: (_, record) => (
        <Space>
          <Button
            icon={<EditOutlined />}
            size="small"
            disabled={!canManage}
            onClick={() => {
              setEditingInstance(record);
              instanceForm.setFieldsValue({
                instance_id: record.instance_id,
                name: record.name,
                description_short: record.description_short,
                status: record.status,
                default_entry_path: record.default_entry_path
              });
              setInstanceModalOpen(true);
            }}
          />
          <Popconfirm
            title={i18nText('settings', 'auto.mcp_hard_delete_confirm')}
            disabled={!canManage}
            onConfirm={() => deleteInstanceMutation.mutate(record.instance_id)}
          >
            <Button
              danger
              icon={<DeleteOutlined />}
              size="small"
              disabled={!canManage}
            />
          </Popconfirm>
        </Space>
      )
    }
  ];

  return (
    <Space direction="vertical" size="middle" className="mcp-management__stack">
      <Flex justify="space-between" align="center">
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.mcp_instances_hint')}
        </Typography.Text>
        <Space>
          <Button
            icon={<DownloadOutlined />}
            loading={exportingInstances}
            onClick={handleExportInstances}
          >
            {i18nText('settings', 'auto.export')}
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            disabled={!canManage}
            onClick={() => {
              setEditingInstance(null);
              instanceForm.setFieldsValue({
                instance_id: '',
                name: '',
                description_short: null,
                status: 'draft',
                default_entry_path: '/'
              });
              setInstanceModalOpen(true);
            }}
          >
            {i18nText('settings', 'auto.new')}
          </Button>
        </Space>
      </Flex>
      <Table
        rowKey="id"
        columns={instanceColumns}
        dataSource={catalog.instances}
        pagination={false}
      />
      {selectedInstance ? (
        <>
          <Divider />
          <Flex justify="space-between" align="center" wrap="wrap" gap={12}>
            <Typography.Title level={5}>
              {i18nText('settings', 'auto.directory_editor')}
            </Typography.Title>
            <Select
              className="mcp-management__instance-select"
              value={selectedInstance.instance_id}
              options={catalog.instances.map((instance) => ({
                label: `${instance.name} (${instance.instance_id})`,
                value: instance.instance_id
              }))}
              onChange={(value) => {
                setRequestedInstanceId(value);
                groupForm.setFieldValue('instance_id', value);
                bindingForm.setFieldValue('instance_id', value);
              }}
            />
          </Flex>
          <Flex gap={16} align="flex-start" wrap="wrap">
            <div className="mcp-management__directory-tree">
              <Tree blockNode defaultExpandAll treeData={directoryTreeData} />
            </div>
            <div className="mcp-management__directory-config">
              <Flex gap={16} align="flex-start" wrap="wrap">
                <Form
                  form={groupForm}
                  layout="vertical"
                  className="mcp-management__form-pane"
                  initialValues={{
                    instance_id: selectedInstance.instance_id,
                    enabled: true,
                    sort_order: 0
                  }}
                  onFinish={(values) => saveGroupMutation.mutate(values)}
                >
                  <Typography.Text strong>
                    {i18nText('settings', 'auto.add_group')}
                  </Typography.Text>
                  <Form.Item
                    name="instance_id"
                    label="instance_id"
                    rules={[{ required: true }]}
                  >
                    <Select
                      options={catalog.instances.map((instance) => ({
                        label: instance.name,
                        value: instance.instance_id
                      }))}
                    />
                  </Form.Item>
                  <Form.Item
                    name="path"
                    label="path"
                    rules={[{ required: true }]}
                  >
                    <Input placeholder="/ops" />
                  </Form.Item>
                  <Form.Item
                    name="display_name"
                    label="display_name"
                    rules={[{ required: true }]}
                  >
                    <Input />
                  </Form.Item>
                  <Form.Item name="description_short" label="description_short">
                    <Input />
                  </Form.Item>
                  <Form.Item
                    name="enabled"
                    label="enabled"
                    valuePropName="checked"
                  >
                    <Switch />
                  </Form.Item>
                  <Form.Item name="sort_order" label="sort_order">
                    <InputNumber />
                  </Form.Item>
                  <Button
                    htmlType="submit"
                    icon={<SaveOutlined />}
                    disabled={!canManage}
                    loading={saveGroupMutation.isPending}
                  >
                    {i18nText('settings', 'auto.save')}
                  </Button>
                </Form>
                <Form
                  form={bindingForm}
                  layout="vertical"
                  className="mcp-management__form-pane"
                  initialValues={{
                    instance_id: selectedInstance.instance_id,
                    visible: true,
                    sort_order: 0
                  }}
                  onFinish={(values) => saveBindingMutation.mutate(values)}
                >
                  <Typography.Text strong>
                    {editingBinding
                      ? i18nText('settings', 'auto.edit_tool_binding')
                      : i18nText('settings', 'auto.add_tool_binding')}
                  </Typography.Text>
                  <Form.Item
                    name="instance_id"
                    label="instance_id"
                    rules={[{ required: true }]}
                  >
                    <Select
                      disabled={Boolean(editingBinding)}
                      options={catalog.instances.map((instance) => ({
                        label: instance.name,
                        value: instance.instance_id
                      }))}
                    />
                  </Form.Item>
                  <Form.Item
                    name="group_path"
                    label="group_path"
                    rules={[{ required: true }]}
                  >
                    <Input placeholder="/ops" />
                  </Form.Item>
                  <Form.Item
                    name="tool_id"
                    label="tool_id"
                    rules={[{ required: true }]}
                  >
                    <Select
                      disabled={Boolean(editingBinding)}
                      options={catalog.tools.map((tool) => ({
                        label: tool.name,
                        value: tool.tool_id
                      }))}
                    />
                  </Form.Item>
                  <Form.Item name="display_alias" label="display_alias">
                    <Input />
                  </Form.Item>
                  <Form.Item
                    name="visible"
                    label="visible"
                    valuePropName="checked"
                  >
                    <Switch />
                  </Form.Item>
                  <Form.Item name="sort_order" label="sort_order">
                    <InputNumber />
                  </Form.Item>
                  <Space>
                    <Button
                      htmlType="submit"
                      icon={<SaveOutlined />}
                      disabled={!canManage}
                      loading={saveBindingMutation.isPending}
                    >
                      {i18nText('settings', 'auto.save')}
                    </Button>
                    {editingBinding ? (
                      <Button
                        onClick={() => {
                          setEditingBinding(null);
                          bindingForm.resetFields();
                          bindingForm.setFieldValue(
                            'instance_id',
                            selectedInstance.instance_id
                          );
                        }}
                      >
                        {i18nText('settings', 'auto.cancel')}
                      </Button>
                    ) : null}
                  </Space>
                </Form>
              </Flex>
            </div>
          </Flex>
          <Table
            rowKey="id"
            size="small"
            columns={[
              { title: 'path', dataIndex: 'path' },
              { title: 'display_name', dataIndex: 'display_name' },
              {
                title: 'enabled',
                dataIndex: 'enabled',
                render: (value) => String(value)
              },
              {
                title: i18nText('settings', 'auto.operation'),
                render: (_, record) => (
                  <Space>
                    <Button
                      icon={<EditOutlined />}
                      size="small"
                      disabled={!canManage}
                      onClick={() => {
                        const instance = catalog.instances.find(
                          (item) => item.id === record.instance_record_id
                        );
                        groupForm.setFieldsValue({
                          instance_id:
                            instance?.instance_id ??
                            selectedInstance.instance_id,
                          path: record.path,
                          display_name: record.display_name,
                          description_short: record.description_short,
                          enabled: record.enabled,
                          sort_order: record.sort_order
                        });
                      }}
                    />
                    <Popconfirm
                      title={i18nText(
                        'settings',
                        'auto.mcp_hard_delete_confirm'
                      )}
                      disabled={!canManage}
                      onConfirm={() => {
                        const instance = catalog.instances.find(
                          (item) => item.id === record.instance_record_id
                        );
                        deleteGroupMutation.mutate({
                          instanceId:
                            instance?.instance_id ??
                            selectedInstance.instance_id,
                          path: record.path
                        });
                      }}
                    >
                      <Button
                        danger
                        icon={<DeleteOutlined />}
                        size="small"
                        disabled={!canManage}
                      />
                    </Popconfirm>
                  </Space>
                )
              }
            ]}
            dataSource={catalog.groups}
            pagination={false}
          />
          <Table
            rowKey="id"
            size="small"
            columns={[
              { title: 'group_path', dataIndex: 'group_path' },
              { title: 'tool_id', dataIndex: 'tool_id' },
              { title: 'display_alias', dataIndex: 'display_alias' },
              {
                title: 'visible',
                dataIndex: 'visible',
                render: (value) => String(value)
              },
              {
                title: i18nText('settings', 'auto.operation'),
                render: (_, record) => (
                  <Space>
                    <Button
                      icon={<EditOutlined />}
                      size="small"
                      disabled={!canManage}
                      onClick={() => {
                        setEditingBinding(record);
                        bindingForm.setFieldsValue({
                          instance_id: resolveInstanceId(record),
                          group_path: record.group_path,
                          tool_id: record.tool_id,
                          display_alias: record.display_alias,
                          visible: record.visible,
                          sort_order: record.sort_order
                        });
                      }}
                    />
                    <Popconfirm
                      title={i18nText(
                        'settings',
                        'auto.mcp_hard_delete_confirm'
                      )}
                      disabled={!canManage}
                      onConfirm={() => deleteBindingMutation.mutate(record.id)}
                    >
                      <Button
                        danger
                        icon={<DeleteOutlined />}
                        size="small"
                        disabled={!canManage}
                      />
                    </Popconfirm>
                  </Space>
                )
              }
            ]}
            dataSource={catalog.bindings}
            pagination={false}
          />
        </>
      ) : null}
      <Modal
        open={instanceModalOpen}
        title={
          editingInstance
            ? i18nText('settings', 'auto.edit')
            : i18nText('settings', 'auto.new')
        }
        onCancel={() => setInstanceModalOpen(false)}
        onOk={() => instanceForm.submit()}
        confirmLoading={saveInstanceMutation.isPending}
      >
        <Form
          form={instanceForm}
          layout="vertical"
          onFinish={(values) => saveInstanceMutation.mutate(values)}
        >
          <Form.Item
            name="instance_id"
            label="instance_id"
            rules={[{ required: true }]}
          >
            <Input disabled={Boolean(editingInstance)} />
          </Form.Item>
          <Form.Item name="name" label="name" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="description_short" label="description_short">
            <Input />
          </Form.Item>
          <Form.Item name="status" label="status" rules={[{ required: true }]}>
            <Select
              options={['draft', 'enabled', 'disabled', 'archived'].map(
                (value) => ({ label: value, value })
              )}
            />
          </Form.Item>
          <Form.Item
            name="default_entry_path"
            label="default_entry_path"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}

function McpToolsTab({
  canManage,
  catalog,
  interfaceCapabilities
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
  interfaceCapabilities: ConsoleMcpInterfaceCapability[];
}) {
  const csrfToken = useCsrfToken();
  const queryClient = useQueryClient();
  const [form] = Form.useForm<ToolFormValues>();
  const [toolsState, dispatchToolsState] = useReducer(
    mcpToolsReducer,
    initialMcpToolsState
  );
  const {
    modalOpen,
    editingTool,
    step,
    keyword,
    pathFilter,
    interfaceId,
    riskLevel,
    status,
    desIdRequired,
    exportingCatalog
  } = toolsState;
  const setModalOpen = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchToolsState({ type: 'setModalOpen', value }),
    []
  );
  const setEditingTool = useCallback(
    (value: SetStateAction<ConsoleMcpTool | null>) =>
      dispatchToolsState({ type: 'setEditingTool', value }),
    []
  );
  const setStep = useCallback(
    (value: SetStateAction<string>) =>
      dispatchToolsState({ type: 'setStep', value }),
    []
  );
  const setKeyword = useCallback(
    (value: SetStateAction<string>) =>
      dispatchToolsState({ type: 'setKeyword', value }),
    []
  );
  const setPathFilter = useCallback(
    (value: SetStateAction<string>) =>
      dispatchToolsState({ type: 'setPathFilter', value }),
    []
  );
  const setInterfaceId = useCallback(
    (value: SetStateAction<string | undefined>) =>
      dispatchToolsState({ type: 'setInterfaceId', value }),
    []
  );
  const setRiskLevel = useCallback(
    (value: SetStateAction<string | undefined>) =>
      dispatchToolsState({ type: 'setRiskLevel', value }),
    []
  );
  const setStatus = useCallback(
    (value: SetStateAction<string | undefined>) =>
      dispatchToolsState({ type: 'setStatus', value }),
    []
  );
  const setDesIdRequired = useCallback(
    (value: SetStateAction<boolean | undefined>) =>
      dispatchToolsState({ type: 'setDesIdRequired', value }),
    []
  );
  const setExportingCatalog = useCallback(
    (value: SetStateAction<boolean>) =>
      dispatchToolsState({ type: 'setExportingCatalog', value }),
    []
  );
  const autoGeneratedToolIdRef = useRef('');
  const inputMappingValidRef = useRef(true);
  const outputMappingValidRef = useRef(true);
  const [schemaEditorRevision, bumpSchemaEditorRevision] = useReducer(
    (value: number) => value + 1,
    0
  );
  const setInputMappingValue = useCallback(
    (mapping: McpInputMappingValue) =>
      form.setFieldValue('input_mapping', mapping),
    [form]
  );
  const setOutputMappingValue = useCallback(
    (schema: Record<string, unknown>) =>
      form.setFieldValue('output_mapping', schema),
    [form]
  );
  const setInputMappingValidity = useCallback((valid: boolean) => {
    inputMappingValidRef.current = valid;
  }, []);
  const setOutputMappingValidity = useCallback((valid: boolean) => {
    outputMappingValidRef.current = valid;
  }, []);
  const columns = useMemo<Array<DataTableColumn<ConsoleMcpTool>>>(
    () => [
      {
        key: 'name',
        title: i18nText('settings', 'auto.tool_name'),
        dataIndex: 'name',
        width: 220,
        render: (_, record) => (
          <Space direction="vertical" size={0}>
            <Typography.Text strong>{record.name}</Typography.Text>
            <Typography.Text type="secondary">{record.tool_id}</Typography.Text>
          </Space>
        )
      },
      {
        key: 'interface_id',
        title: 'interface_id',
        dataIndex: 'interface_id',
        width: 240,
        ellipsis: true
      },
      {
        key: 'risk_level',
        title: 'risk_level',
        dataIndex: 'risk_level',
        width: 120,
        render: (value) => (
          <Tag color={riskColor(String(value))}>{String(value)}</Tag>
        )
      },
      {
        key: 'des_id',
        title: 'des_id',
        dataIndex: 'des_id',
        width: 140
      },
      {
        key: 'status',
        title: 'status',
        dataIndex: 'status',
        width: 120,
        render: (value) => (
          <Tag color={statusColor(String(value))}>{String(value)}</Tag>
        )
      }
    ],
    []
  );
  const saveToolMutation = useMutation({
    mutationFn: (values: ToolFormValues) => {
      if (!inputMappingValidRef.current) {
        throw new Error('input_mapping JSON');
      }
      if (!outputMappingValidRef.current) {
        throw new Error('output_mapping JSON');
      }
      const selectedInterface = interfaceCapabilities.find(
        (entry) => entry.interface_id === values.interface_id
      );
      const inputMapping = normalizeInputMapping(
        form.getFieldValue('input_mapping')
      );
      const outputMapping = schemaRecord(form.getFieldValue('output_mapping'));
      const body: SaveConsoleMcpToolBody = {
        tool_id: editingTool ? editingTool.tool_id : values.tool_id,
        des_id: values.des_id,
        name: values.name,
        short_description: values.short_description,
        usage_description: values.usage_description,
        full_description: values.full_description,
        interface_id: values.interface_id,
        parameter_schema: selectedInterface?.parameter_schema ?? {},
        result_schema: selectedInterface?.result_schema ?? {},
        input_mapping: inputMapping,
        output_mapping: outputMapping,
        permission_code: selectedInterface?.permission_code ?? null,
        risk_level: selectedInterface?.risk_level ?? 'medium',
        audit_policy: parseJsonText(values.audit_policy_text, 'audit_policy'),
        des_id_required: true,
        status: values.status
      };
      if (editingTool) {
        const updateBody = {
          name: body.name,
          des_id: body.des_id,
          short_description: body.short_description,
          usage_description: body.usage_description,
          full_description: body.full_description,
          interface_id: body.interface_id,
          parameter_schema: body.parameter_schema,
          result_schema: body.result_schema,
          input_mapping: body.input_mapping,
          output_mapping: body.output_mapping,
          permission_code: body.permission_code,
          risk_level: body.risk_level,
          audit_policy: body.audit_policy,
          des_id_required: body.des_id_required,
          status: body.status
        };
        return updateSettingsMcpTool(
          editingTool.tool_id,
          updateBody,
          csrfToken
        );
      }
      return createSettingsMcpTool(body, csrfToken);
    },
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      setModalOpen(false);
      setEditingTool(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });
  const deleteToolMutation = useMutation({
    mutationFn: (toolId: string) => deleteSettingsMcpTool(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const refreshMutation = useMutation({
    mutationFn: (toolId: string) =>
      refreshSettingsMcpToolDescription(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_des_id_refreshed'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteToolMutationRef = useRef(deleteToolMutation);
  const refreshMutationRef = useRef(refreshMutation);

  useEffect(() => {
    deleteToolMutationRef.current = deleteToolMutation;
    refreshMutationRef.current = refreshMutation;
  }, [deleteToolMutation, refreshMutation]);

  async function handleExportCatalog() {
    setExportingCatalog(true);
    try {
      const exportPackage = await exportSettingsMcpCatalog();
      downloadMcpExportPackage(exportPackage);
      message.success(i18nText('settings', 'auto.mcp_export_ready'));
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingCatalog(false);
    }
  }
  function applyInterfaceToMapping(
    field: 'input_mapping' | 'output_mapping',
    entry: ConsoleMcpInterfaceCapability | undefined
  ) {
    if (!entry) {
      return;
    }

    const nextMapping =
      field === 'input_mapping'
        ? buildInputMappingFromInterface(entry, form.getFieldValue(field))
        : schemaRecord(entry.result_schema);
    const currentHasContent =
      field === 'input_mapping'
        ? inputMappingHasContent(form.getFieldValue(field))
        : schemaMappingHasContent(form.getFieldValue(field));
    const applyMapping = () => {
      form.setFieldValue(field, nextMapping);
      if (field === 'input_mapping') {
        inputMappingValidRef.current = true;
      } else {
        outputMappingValidRef.current = true;
      }
      bumpSchemaEditorRevision();
    };

    if (!currentHasContent) {
      applyMapping();
      return;
    }

    Modal.confirm({
      title: i18nText('settings', 'auto.mcp_mapping_overwrite_confirm_title'),
      content: i18nText(
        'settings',
        'auto.mcp_mapping_overwrite_confirm_content'
      ),
      okText: i18nText('settings', 'auto.confirm'),
      cancelText: i18nText('settings', 'auto.cancel'),
      onOk: applyMapping
    });
  }
  const bindingPathsByToolId = useMemo(() => {
    const paths = new Map<string, Set<string>>();

    for (const binding of catalog.bindings) {
      const current = paths.get(binding.tool_id) ?? new Set<string>();
      current.add(binding.group_path);
      paths.set(binding.tool_id, current);
    }

    return new Map(
      Array.from(paths, ([toolId, groupPaths]) => [
        toolId,
        Array.from(groupPaths)
      ])
    );
  }, [catalog.bindings]);

  const filteredTools = catalog.tools.filter((tool) => {
    const text =
      `${tool.name} ${tool.tool_id} ${tool.interface_id}`.toLowerCase();
    const paths = bindingPathsByToolId.get(tool.tool_id) ?? [];
    return (
      (!keyword || text.includes(keyword.toLowerCase())) &&
      (!pathFilter ||
        paths.some((path) =>
          path.toLowerCase().includes(pathFilter.toLowerCase())
        )) &&
      (!interfaceId || tool.interface_id === interfaceId) &&
      (!riskLevel || tool.risk_level === riskLevel) &&
      (!status || tool.status === status) &&
      (desIdRequired === undefined || tool.des_id_required === desIdRequired)
    );
  });
  const toolStepIndex = Math.max(
    0,
    TOOL_FORM_STEPS.findIndex((entry) => entry.value === step)
  );
  const previousToolStep = TOOL_FORM_STEPS[toolStepIndex - 1];
  const nextToolStep = TOOL_FORM_STEPS[toolStepIndex + 1];

  const tableColumns = useMemo<Array<DataTableColumn<ConsoleMcpTool>>>(
    () => [
      ...columns,
      {
        key: 'paths',
        title: 'group_path',
        width: 180,
        render: (_, record) => (
          <Space wrap size={[4, 4]}>
            {(bindingPathsByToolId.get(record.tool_id) ?? ['/']).map((path) => (
              <Tag key={path}>{path}</Tag>
            ))}
          </Space>
        )
      },
      {
        key: 'actions',
        title: i18nText('settings', 'auto.operation'),
        width: 180,
        render: (_, record) => (
          <Space>
            <Button
              icon={<EditOutlined />}
              size="small"
              disabled={!canManage}
              onClick={() => {
                autoGeneratedToolIdRef.current = '';
                inputMappingValidRef.current = true;
                outputMappingValidRef.current = true;
                setEditingTool(record);
                setStep('basic');
                form.setFieldsValue({
                  tool_id: record.tool_id,
                  name: record.name,
                  short_description: record.short_description,
                  usage_description: record.usage_description,
                  full_description: record.full_description,
                  des_id: record.des_id,
                  interface_id: record.interface_id,
                  audit_policy_text: stringifyJson(record.audit_policy),
                  status: record.status
                });
                form.setFieldValue(
                  'input_mapping',
                  normalizeInputMapping(record.input_mapping)
                );
                form.setFieldValue(
                  'output_mapping',
                  schemaRecord(record.output_mapping)
                );
                bumpSchemaEditorRevision();
                setModalOpen(true);
              }}
            />
            <Button
              icon={<ReloadOutlined />}
              size="small"
              disabled={!canManage}
              loading={refreshMutation.isPending}
              onClick={() => refreshMutationRef.current.mutate(record.tool_id)}
            />
            <Popconfirm
              title={i18nText('settings', 'auto.mcp_hard_delete_confirm')}
              disabled={!canManage}
              onConfirm={() =>
                deleteToolMutationRef.current.mutate(record.tool_id)
              }
            >
              <Button
                danger
                icon={<DeleteOutlined />}
                size="small"
                disabled={!canManage}
              />
            </Popconfirm>
          </Space>
        )
      }
    ],
    [
      bindingPathsByToolId,
      canManage,
      columns,
      form,
      refreshMutation.isPending,
      setEditingTool,
      setModalOpen,
      setStep
    ]
  );
  const configuration = useUserPreferenceDataTableConfiguration<ConsoleMcpTool>(
    {
      preferenceKey: 'settings.mcp-management.tools',
      columns: tableColumns
    }
  );

  return (
    <Space direction="vertical" size="middle" className="mcp-management__stack">
      <Flex justify="space-between" align="center" wrap="wrap" gap={12}>
        <Space wrap>
          <Input.Search
            allowClear
            placeholder="keyword / tool_id / interface_id"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
          />
          <Input
            allowClear
            placeholder="group_path"
            value={pathFilter}
            onChange={(event) => setPathFilter(event.target.value)}
          />
          <Select
            allowClear
            showSearch
            optionFilterProp="label"
            placeholder="interface_id"
            value={interfaceId}
            options={interfaceCapabilities.map((entry) => ({
              label: `${interfaceOptionLabel(entry)} ${entry.interface_id}`,
              value: entry.interface_id
            }))}
            onChange={setInterfaceId}
          />
          <Select
            allowClear
            placeholder="risk_level"
            value={riskLevel}
            options={['low', 'medium', 'high', 'critical'].map((value) => ({
              label: value,
              value
            }))}
            onChange={setRiskLevel}
          />
          <Select
            allowClear
            placeholder="des_id_required"
            value={desIdRequired}
            options={[
              { label: 'true', value: true },
              { label: 'false', value: false }
            ]}
            onChange={setDesIdRequired}
          />
          <Select
            allowClear
            placeholder="status"
            value={status}
            options={['draft', 'enabled', 'disabled', 'archived'].map(
              (value) => ({
                label: value,
                value
              })
            )}
            onChange={setStatus}
          />
        </Space>
        <Space>
          <DataTableColumnSettings
            columns={tableColumns}
            configuration={configuration}
          />
          <Button
            icon={<DownloadOutlined />}
            onClick={handleExportCatalog}
            loading={exportingCatalog}
          >
            {i18nText('settings', 'auto.export')}
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            disabled={!canManage}
            onClick={() => {
              autoGeneratedToolIdRef.current = '';
              inputMappingValidRef.current = true;
              outputMappingValidRef.current = true;
              setEditingTool(null);
              setStep('basic');
              form.setFieldsValue({
                tool_id: '',
                name: '',
                short_description: '',
                usage_description: '',
                full_description: '',
                des_id: buildRandomToolIdSeed(),
                interface_id: undefined,
                audit_policy_text: '{"enabled":true}',
                status: 'draft'
              });
              form.setFieldValue('input_mapping', {
                interface_parameters: [],
                mappings: []
              });
              form.setFieldValue('output_mapping', emptyObjectSchema());
              bumpSchemaEditorRevision();
              setModalOpen(true);
            }}
          >
            {i18nText('settings', 'auto.new')}
          </Button>
        </Space>
      </Flex>
      <DataTable
        columns={tableColumns}
        configuration={configuration}
        dataSource={filteredTools}
        page={1}
        pageSize={Math.max(filteredTools.length, 1)}
        total={filteredTools.length}
        rowKey="id"
        onPageChange={() => undefined}
      />
      <Modal
        width={840}
        className="mcp-management__tool-modal"
        centered
        open={modalOpen}
        title={
          editingTool
            ? i18nText('settings', 'auto.edit')
            : i18nText('settings', 'auto.new')
        }
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saveToolMutation.isPending}
        footer={
          <Space>
            {previousToolStep ? (
              <Button
                icon={<LeftOutlined />}
                disabled={saveToolMutation.isPending}
                onClick={() => setStep(previousToolStep.value)}
              >
                上一步
              </Button>
            ) : null}
            {nextToolStep ? (
              <Button
                icon={<RightOutlined />}
                disabled={saveToolMutation.isPending}
                onClick={() => setStep(nextToolStep.value)}
              >
                下一步
              </Button>
            ) : null}
            <Button onClick={() => setModalOpen(false)}>Cancel</Button>
            <Button
              type="primary"
              loading={saveToolMutation.isPending}
              onClick={() => form.submit()}
            >
              OK
            </Button>
          </Space>
        }
      >
        <Steps
          size="small"
          current={toolStepIndex}
          items={TOOL_FORM_STEPS.map((entry) => ({
            title: entry.title
          }))}
        />
        <Segmented
          block
          className="mcp-management__segmented"
          value={step}
          options={TOOL_FORM_STEPS.map((entry) => ({
            label: entry.label,
            value: entry.value
          }))}
          onChange={(value) => setStep(String(value))}
        />
        <Form
          form={form}
          className="mcp-management__tool-form"
          layout="vertical"
          onFinish={(values) => saveToolMutation.mutate(values)}
          onValuesChange={(changedValues, values) => {
            if (editingTool || !('name' in changedValues)) {
              return;
            }

            const currentToolId = values.tool_id ?? '';
            if (
              currentToolId &&
              currentToolId !== autoGeneratedToolIdRef.current
            ) {
              return;
            }

            const generatedToolId = buildReadableToolId(values.name ?? '');
            autoGeneratedToolIdRef.current = generatedToolId;
            form.setFieldValue('tool_id', generatedToolId);
          }}
        >
          <div hidden={step !== 'basic'}>
            <Form.Item name="name" label="name" rules={[{ required: true }]}>
              <Input />
            </Form.Item>
            <Form.Item
              name="tool_id"
              label="tool_id"
              rules={[{ required: true, whitespace: true }]}
            >
              <Input
                disabled={Boolean(editingTool)}
                addonAfter={
                  editingTool ? undefined : (
                    <Tooltip title="随机生成 tool_id">
                      <Button
                        type="text"
                        htmlType="button"
                        size="small"
                        icon={<ReloadOutlined />}
                        onClick={() => {
                          autoGeneratedToolIdRef.current = '';
                          form.setFieldValue(
                            'tool_id',
                            buildReadableToolId('', buildRandomToolIdSeed())
                          );
                        }}
                      />
                    </Tooltip>
                  )
                }
              />
            </Form.Item>
            <Form.Item
              name="des_id"
              label="des_id"
              rules={[{ required: true, whitespace: true }]}
            >
              <Input
                addonAfter={
                  <Tooltip title="随机生成 des_id">
                    <Button
                      type="text"
                      htmlType="button"
                      size="small"
                      icon={<ReloadOutlined />}
                      onClick={() => {
                        form.setFieldValue('des_id', buildRandomToolIdSeed());
                      }}
                    />
                  </Tooltip>
                }
              />
            </Form.Item>
            <Form.Item
              name="short_description"
              label="short_description"
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="status"
              label="status"
              rules={[{ required: true }]}
            >
              <Select
                options={['draft', 'enabled', 'disabled', 'archived'].map(
                  (value) => ({ label: value, value })
                )}
              />
            </Form.Item>
          </div>
          <div hidden={step !== 'interface'}>
            <Form.Item
              name="interface_id"
              label="interface_id"
              rules={[{ required: true }]}
            >
              <Select
                showSearch
                optionFilterProp="label"
                options={interfaceCapabilities.map((entry) => ({
                  label: `${interfaceOptionLabel(entry)} - ${entry.interface_id}${
                    entry.bindable ? '' : ` (${entry.disabled_reason})`
                  }`,
                  value: entry.interface_id,
                  disabled: !entry.bindable
                }))}
              />
            </Form.Item>
            <Form.Item
              noStyle
              shouldUpdate={(previous, current) =>
                previous.interface_id !== current.interface_id
              }
            >
              {({ getFieldValue }) => {
                const selectedInterface = interfaceCapabilities.find(
                  (entry) =>
                    entry.interface_id === getFieldValue('interface_id')
                );

                if (!selectedInterface) {
                  return null;
                }

                return (
                  <Descriptions bordered size="small" column={1}>
                    <Descriptions.Item label="operation">
                      {interfaceOptionLabel(selectedInterface)}
                    </Descriptions.Item>
                    <Descriptions.Item label="operationId">
                      {selectedInterface.interface_id}
                    </Descriptions.Item>
                    <Descriptions.Item label="risk_level">
                      {selectedInterface.risk_level}
                    </Descriptions.Item>
                    <Descriptions.Item label="permission_code">
                      {selectedInterface.permission_code ?? '-'}
                    </Descriptions.Item>
                  </Descriptions>
                );
              }}
            </Form.Item>
          </div>
          {step === 'input' ? (
            <div>
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.interface_id !== current.interface_id ||
                  previous.input_mapping !== current.input_mapping
                }
              >
                {({ getFieldValue }) => {
                  const selectedInterface = interfaceCapabilities.find(
                    (entry) =>
                      entry.interface_id === getFieldValue('interface_id')
                  );

                  return (
                    <Flex justify="space-between" align="center" gap={12}>
                      <SelectedInterfaceOperationTitle
                        selectedInterface={selectedInterface}
                      />
                      <Button
                        disabled={!selectedInterface}
                        onClick={() =>
                          applyInterfaceToMapping(
                            'input_mapping',
                            selectedInterface
                          )
                        }
                      >
                        {i18nText(
                          'settings',
                          'auto.mcp_get_interface_parameters'
                        )}
                      </Button>
                    </Flex>
                  );
                }}
              </Form.Item>
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.input_mapping !== current.input_mapping
                }
              >
                {({ getFieldValue }) => (
                  <div className="mcp-management__input-mapping-editor">
                    <McpInputMappingEditor
                      resetKey={`input:${schemaEditorRevision}`}
                      value={getFieldValue('input_mapping')}
                      onChange={setInputMappingValue}
                      onValidityChange={setInputMappingValidity}
                    />
                  </div>
                )}
              </Form.Item>
            </div>
          ) : null}
          {step === 'output' ? (
            <div>
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.interface_id !== current.interface_id ||
                  previous.output_mapping !== current.output_mapping
                }
              >
                {({ getFieldValue }) => {
                  const selectedInterface = interfaceCapabilities.find(
                    (entry) =>
                      entry.interface_id === getFieldValue('interface_id')
                  );

                  return (
                    <Flex justify="space-between" align="center" gap={12}>
                      <SelectedInterfaceOperationTitle
                        selectedInterface={selectedInterface}
                      />
                      <Button
                        disabled={!selectedInterface}
                        onClick={() =>
                          applyInterfaceToMapping(
                            'output_mapping',
                            selectedInterface
                          )
                        }
                      >
                        {i18nText(
                          'settings',
                          'auto.mcp_get_interface_result'
                        )}
                      </Button>
                    </Flex>
                  );
                }}
              </Form.Item>
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.output_mapping !== current.output_mapping
                }
              >
                {({ getFieldValue }) => (
                  <div className="mcp-management__schema-editor">
                    <JsonSchemaInlineEditor
                      fallbackRootType="object"
                      resetKey={`output:${schemaEditorRevision}`}
                      schema={schemaRecord(getFieldValue('output_mapping'))}
                      onChange={setOutputMappingValue}
                      onValidityChange={setOutputMappingValidity}
                    />
                  </div>
                )}
              </Form.Item>
            </div>
          ) : null}
          <div hidden={step !== 'description'}>
            <Form.Item
              noStyle
              shouldUpdate={(previous, current) =>
                previous.interface_id !== current.interface_id
              }
            >
              {({ getFieldValue }) => {
                const selectedInterface = interfaceCapabilities.find(
                  (entry) =>
                    entry.interface_id === getFieldValue('interface_id')
                );

                return (
                  <div className="mcp-management__selected-operation">
                    <SelectedInterfaceOperationTitle
                      selectedInterface={selectedInterface}
                    />
                  </div>
                );
              }}
            </Form.Item>
            <Form.Item name="usage_description" label="usage_description">
              <Input.TextArea rows={3} />
            </Form.Item>
            <Form.Item
              name="full_description"
              label="full_description"
              rules={[{ required: true }]}
            >
              <Input.TextArea rows={6} />
            </Form.Item>
            <Form.Item
              name="audit_policy_text"
              label="audit_policy"
              rules={[{ required: true }]}
            >
              <Input.TextArea rows={4} />
            </Form.Item>
            <Form.Item noStyle shouldUpdate>
              {({ getFieldsValue }) => {
                const values = getFieldsValue() as ToolFormValues;

                return (
                  <Descriptions bordered size="small" column={1}>
                    <Descriptions.Item label="mcp.get(tool_id)">
                      {values.tool_id || autoGeneratedToolIdRef.current}
                    </Descriptions.Item>
                    <Descriptions.Item label="interface_id">
                      {values.interface_id}
                    </Descriptions.Item>
                  </Descriptions>
                );
              }}
            </Form.Item>
          </div>
        </Form>
      </Modal>
    </Space>
  );
}

function McpMetaConfigTab({
  canManage,
  metaToolConfig
}: {
  canManage: boolean;
  metaToolConfig: ConsoleMcpMetaToolConfig;
}) {
  const csrfToken = useCsrfToken();
  const queryClient = useQueryClient();
  const [form] = Form.useForm<MetaToolConfigFormValues>();
  const initialValues = useMemo(
    () => ({
      ...metaToolConfig,
      list_return_fields_text: stringifyJson(metaToolConfig.list_return_fields)
    }),
    [metaToolConfig]
  );

  useEffect(() => {
    form.setFieldsValue(initialValues);
  }, [form, initialValues]);

  const saveMutation = useMutation({
    mutationFn: (values: MetaToolConfigFormValues) =>
      updateSettingsMcpMetaToolConfig(
        {
          list_default_limit: values.list_default_limit,
          list_max_depth: values.list_max_depth,
          list_regex_enabled: values.list_regex_enabled,
          list_regex_max_length: values.list_regex_max_length,
          list_return_fields: parseJsonText(
            values.list_return_fields_text,
            'list_return_fields'
          ),
          get_include_mapping_summary: values.get_include_mapping_summary,
          get_include_interface_summary: values.get_include_interface_summary,
          call_default_des_id_policy: values.call_default_des_id_policy,
          call_high_risk_requires_des_id: values.call_high_risk_requires_des_id,
          call_validation_error_format: values.call_validation_error_format
        },
        csrfToken
      ),
    onSuccess: () => {
      message.success(i18nText('settings', 'auto.mcp_saved'));
      void queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });

  return (
    <Space direction="vertical" size="middle" className="mcp-management__stack">
      <Descriptions bordered size="small" column={1}>
        <Descriptions.Item label="mcp.list">
          limit / depth / regex / return fields
        </Descriptions.Item>
        <Descriptions.Item label="mcp.get">
          mapping summary / interface summary
        </Descriptions.Item>
        <Descriptions.Item label="mcp.call">
          des_id policy / high risk policy / validation errors
        </Descriptions.Item>
      </Descriptions>
      <Form
        form={form}
        layout="vertical"
        initialValues={initialValues}
        onFinish={(values) => saveMutation.mutate(values)}
        className="mcp-management__meta-form"
      >
        <Flex gap={16} wrap="wrap">
          <Form.Item
            name="list_default_limit"
            label="list_default_limit"
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
          <Form.Item
            name="list_max_depth"
            label="list_max_depth"
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
          <Form.Item
            name="list_regex_max_length"
            label="list_regex_max_length"
            rules={[{ required: true }]}
          >
            <InputNumber min={1} />
          </Form.Item>
        </Flex>
        <Form.Item
          name="list_regex_enabled"
          label="list_regex_enabled"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="list_return_fields_text"
          label="list_return_fields"
          rules={[{ required: true }]}
        >
          <Input.TextArea rows={4} />
        </Form.Item>
        <Form.Item
          name="get_include_mapping_summary"
          label="get_include_mapping_summary"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="get_include_interface_summary"
          label="get_include_interface_summary"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="call_default_des_id_policy"
          label="call_default_des_id_policy"
        >
          <Select
            options={['tool_config', 'required', 'optional', 'disabled'].map(
              (value) => ({
                label: value,
                value
              })
            )}
          />
        </Form.Item>
        <Form.Item
          name="call_high_risk_requires_des_id"
          label="call_high_risk_requires_des_id"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="call_validation_error_format"
          label="call_validation_error_format"
        >
          <Select
            options={['structured', 'field_errors'].map((value) => ({
              label: value,
              value
            }))}
          />
        </Form.Item>
        <Button
          type="primary"
          htmlType="submit"
          icon={<SettingOutlined />}
          disabled={!canManage}
          loading={saveMutation.isPending}
        >
          {i18nText('settings', 'auto.save')}
        </Button>
      </Form>
    </Space>
  );
}
