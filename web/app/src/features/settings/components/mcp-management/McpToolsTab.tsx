import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  LeftOutlined,
  PlusOutlined,
  ReloadOutlined,
  RightOutlined
} from '@ant-design/icons';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Descriptions,
  Flex,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Segmented,
  Space,
  Steps,
  Tag,
  Tooltip,
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
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInterfaceCapability,
  ConsoleMcpProxyInputMapping,
  ConsoleMcpProxyOutputMapping,
  ConsoleMcpTool,
  SaveConsoleMcpToolBody
} from '@1flowbase/api-client';

import {
  createSettingsMcpTool,
  deleteSettingsMcpTool,
  executeSettingsMcpToolDebug,
  executeSettingsMcpProxyToolDebug,
  exportSettingsMcpCatalog,
  refreshSettingsMcpToolDescription,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpTool
} from '../../api/mcp-management';
import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import { MarkdownIrEditor } from '../../../../shared/ui/markdown-ir-editor/MarkdownIrEditor';
import { JsonSchemaInlineEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel';
import {
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
import { McpToolDebugPanel } from './McpToolDebugPanel';
import {
  McpProxyMappingEditor,
  mcpProxyMappingIsValid
} from './proxy/McpProxyMappingEditor';
import { McpProxyToolDebugPanel } from './proxy/McpProxyToolDebugPanel';
import { initialMcpToolsState, mcpToolsReducer } from './mcp-management-state';
import {
  downloadMcpExportPackage,
  riskColor,
  statusColor
} from './mcp-management-utils';

type ToolFormValues = {
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  full_description: string;
  execution_target_kind: 'interface_wrapper' | 'mcp_proxy';
  interface_id?: string;
  upstream_connection_id?: string;
  remote_tool_name?: string;
  source_schema_hash?: string;
  input_mapping: McpInputMappingValue | ConsoleMcpProxyInputMapping;
  output_mapping: Record<string, unknown> | ConsoleMcpProxyOutputMapping;
  status: string;
};
const TOOL_FORM_STEPS = [
  { title: 'basic', label: 'basic', value: 'basic' },
  { title: 'interface', label: 'interface', value: 'interface' },
  { title: 'input', label: 'input_mapping', value: 'input' },
  { title: 'output', label: 'output_mapping', value: 'output' },
  { title: 'debug', label: 'debug', value: 'debug' }
];
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

function toolTypeLabel(tool: ConsoleMcpTool) {
  return tool.execution_target.kind === 'mcp_proxy'
    ? i18nText('settingsMcpManagement', 'auto.tool_type_mcp_proxy')
    : i18nText('settingsMcpManagement', 'auto.tool_type_interface_wrapper');
}

function toolSourceLabel(tool: ConsoleMcpTool) {
  return tool.execution_target.kind === 'mcp_proxy'
    ? `${tool.execution_target.upstream_connection_id} / ${tool.execution_target.remote_tool_name}`
    : tool.execution_target.interface_id;
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

export function McpToolsTab({
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
    executionTargetKind,
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
  const setExecutionTargetKind = useCallback(
    (value: SetStateAction<string | undefined>) =>
      dispatchToolsState({ type: 'setExecutionTargetKind', value }),
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
    (mapping: ToolFormValues['input_mapping']) =>
      form.setFieldValue('input_mapping', mapping),
    [form]
  );
  const setOutputMappingValue = useCallback(
    (schema: ToolFormValues['output_mapping']) =>
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
        ellipsis: true
      },
      {
        key: 'tool_id',
        title: 'tool_id',
        dataIndex: 'tool_id',
        width: 180,
        ellipsis: true
      },
      {
        key: 'operation',
        title: 'operation',
        dataIndex: 'operation',
        width: 240,
        ellipsis: true,
        render: (_, record) =>
          record.operation?.trim() ? record.operation : toolSourceLabel(record)
      },
      {
        key: 'execution_target_kind',
        title: i18nText('settingsMcpManagement', 'auto.tool_type'),
        width: 130,
        render: (_, record) => <Tag>{toolTypeLabel(record)}</Tag>
      },
      {
        key: 'execution_source',
        title: i18nText('settingsMcpManagement', 'auto.execution_source'),
        width: 260,
        ellipsis: true,
        render: (_, record) => toolSourceLabel(record)
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
      const common = {
        tool_id: editingTool ? editingTool.tool_id : values.tool_id,
        des_id: values.des_id,
        name: values.name,
        short_description: values.short_description,
        full_description: values.full_description,
        status: values.status
      };
      let body: SaveConsoleMcpToolBody;
      if (editingTool?.execution_target.kind === 'mcp_proxy') {
        const inputMapping = form.getFieldValue(
          'input_mapping'
        ) as ConsoleMcpProxyInputMapping;
        const outputMapping = form.getFieldValue(
          'output_mapping'
        ) as ConsoleMcpProxyOutputMapping;
        if (
          !mcpProxyMappingIsValid(inputMapping) ||
          !mcpProxyMappingIsValid(outputMapping)
        ) {
          throw new Error(
            i18nText('settingsMcpManagement', 'auto.proxy_path_invalid')
          );
        }
        body = {
          ...common,
          execution_target: editingTool.execution_target,
          parameter_schema: editingTool.parameter_schema,
          result_schema: editingTool.result_schema,
          input_mapping: inputMapping,
          output_mapping: outputMapping,
          permission_code: editingTool.permission_code,
          risk_level: editingTool.risk_level
        };
      } else {
        const selectedInterface = interfaceCapabilities.find(
          (entry) => entry.interface_id === values.interface_id
        );
        if (!selectedInterface || !values.interface_id) {
          throw new Error('operation is required');
        }
        body = {
          ...common,
          execution_target: {
            kind: 'interface_wrapper',
            interface_id: values.interface_id
          },
          parameter_schema: selectedInterface.parameter_schema,
          result_schema: selectedInterface.result_schema,
          input_mapping: normalizeInputMapping(
            form.getFieldValue('input_mapping')
          ),
          output_mapping: schemaRecord(form.getFieldValue('output_mapping')),
          permission_code: selectedInterface.permission_code,
          risk_level: selectedInterface.risk_level
        };
      }
      if (editingTool) {
        const { tool_id: _toolId, ...updateBody } = body;
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
  const filteredTools = catalog.tools.filter((tool) => {
    const source = toolSourceLabel(tool);
    const text =
      `${tool.name} ${tool.tool_id} ${tool.operation} ${source}`.toLowerCase();
    return (
      (!keyword || text.includes(keyword.toLowerCase())) &&
      (!executionTargetKind ||
        tool.execution_target.kind === executionTargetKind) &&
      (!interfaceId ||
        (tool.execution_target.kind === 'interface_wrapper' &&
          tool.execution_target.interface_id === interfaceId)) &&
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
                  full_description: record.full_description,
                  des_id: record.des_id,
                  execution_target_kind: record.execution_target.kind,
                  interface_id:
                    record.execution_target.kind === 'interface_wrapper'
                      ? record.execution_target.interface_id
                      : undefined,
                  upstream_connection_id:
                    record.execution_target.kind === 'mcp_proxy'
                      ? record.execution_target.upstream_connection_id
                      : undefined,
                  remote_tool_name:
                    record.execution_target.kind === 'mcp_proxy'
                      ? record.execution_target.remote_tool_name
                      : undefined,
                  source_schema_hash:
                    record.execution_target.kind === 'mcp_proxy'
                      ? record.execution_target.source_schema_hash
                      : undefined,
                  status: record.status
                });
                form.setFieldValue(
                  'input_mapping',
                  record.execution_target.kind === 'mcp_proxy'
                    ? record.input_mapping
                    : normalizeInputMapping(record.input_mapping)
                );
                form.setFieldValue(
                  'output_mapping',
                  record.execution_target.kind === 'mcp_proxy'
                    ? record.output_mapping
                    : schemaRecord(record.output_mapping)
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
      preferenceKey: 'settings.mcp-management.tools.v3',
      columns: tableColumns
    }
  );

  return (
    <Space direction="vertical" size="middle" className="mcp-management__stack">
      <Flex justify="space-between" align="center" wrap="wrap" gap={12}>
        <Space wrap>
          <Input.Search
            allowClear
            placeholder="keyword / tool_id / operation"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
          />
          <Select
            allowClear
            aria-label={i18nText('settingsMcpManagement', 'auto.tool_type')}
            placeholder={i18nText('settingsMcpManagement', 'auto.tool_type')}
            value={executionTargetKind}
            options={[
              {
                label: i18nText(
                  'settingsMcpManagement',
                  'auto.tool_type_interface_wrapper'
                ),
                value: 'interface_wrapper'
              },
              {
                label: i18nText(
                  'settingsMcpManagement',
                  'auto.tool_type_mcp_proxy'
                ),
                value: 'mcp_proxy'
              }
            ]}
            onChange={setExecutionTargetKind}
          />
          <Select
            allowClear
            showSearch
            optionFilterProp="label"
            placeholder="operation"
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
                full_description: '',
                des_id: buildRandomToolIdSeed(),
                execution_target_kind: 'interface_wrapper',
                interface_id: undefined,
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
      <FixedHeightModal
        width={840}
        className="mcp-management__tool-modal"
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
        bodyHeader={
          <>
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
          </>
        }
      >
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
            <Form.Item
              name="short_description"
              label="short_description"
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="full_description"
              label="full_description"
              rules={[{ required: true }]}
            >
              <MarkdownIrEditor ariaLabel="full_description" />
            </Form.Item>
          </div>
          <div hidden={step !== 'interface'}>
            {editingTool?.execution_target.kind === 'mcp_proxy' ? (
              <Descriptions bordered size="small" column={1}>
                <Descriptions.Item
                  label={i18nText(
                    'settingsMcpManagement',
                    'auto.upstream_connection_id'
                  )}
                >
                  {editingTool.execution_target.upstream_connection_id}
                </Descriptions.Item>
                <Descriptions.Item
                  label={i18nText(
                    'settingsMcpManagement',
                    'auto.upstream_remote_tool_name'
                  )}
                >
                  {editingTool.execution_target.remote_tool_name}
                </Descriptions.Item>
                <Descriptions.Item label="source_schema_hash">
                  {editingTool.execution_target.source_schema_hash}
                </Descriptions.Item>
              </Descriptions>
            ) : (
              <>
                <Form.Item
                  name="interface_id"
                  label="operation"
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
              </>
            )}
          </div>
          {step === 'input' ? (
            <div>
              {editingTool?.execution_target.kind === 'mcp_proxy' ? (
                <Form.Item
                  noStyle
                  shouldUpdate={(previous, current) =>
                    previous.input_mapping !== current.input_mapping
                  }
                >
                  {({ getFieldValue }) => (
                    <McpProxyMappingEditor
                      direction="input"
                      value={
                        getFieldValue(
                          'input_mapping'
                        ) as ConsoleMcpProxyInputMapping
                      }
                      onChange={setInputMappingValue}
                      onValidityChange={setInputMappingValidity}
                    />
                  )}
                </Form.Item>
              ) : (
                <>
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
                </>
              )}
            </div>
          ) : null}
          {step === 'output' ? (
            <div>
              {editingTool?.execution_target.kind === 'mcp_proxy' ? (
                <Form.Item
                  noStyle
                  shouldUpdate={(previous, current) =>
                    previous.output_mapping !== current.output_mapping
                  }
                >
                  {({ getFieldValue }) => (
                    <McpProxyMappingEditor
                      direction="output"
                      value={
                        getFieldValue(
                          'output_mapping'
                        ) as ConsoleMcpProxyOutputMapping
                      }
                      onChange={setOutputMappingValue}
                      onValidityChange={setOutputMappingValidity}
                    />
                  )}
                </Form.Item>
              ) : (
                <>
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
                          structureMode="fields"
                          onChange={setOutputMappingValue}
                          onValidityChange={setOutputMappingValidity}
                        />
                      </div>
                    )}
                  </Form.Item>
                </>
              )}
            </div>
          ) : null}
          {step === 'debug' ? (
            editingTool?.execution_target.kind === 'mcp_proxy' ? (
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.input_mapping !== current.input_mapping
                }
              >
                {({ getFieldValue }) => (
                  <McpProxyToolDebugPanel
                    toolId={editingTool.tool_id}
                    csrfToken={csrfToken}
                    inputMapping={
                      getFieldValue(
                        'input_mapping'
                      ) as ConsoleMcpProxyInputMapping
                    }
                    executeDebug={executeSettingsMcpProxyToolDebug}
                  />
                )}
              </Form.Item>
            ) : (
              <Form.Item
                noStyle
                shouldUpdate={(previous, current) =>
                  previous.interface_id !== current.interface_id ||
                  previous.input_mapping !== current.input_mapping ||
                  previous.output_mapping !== current.output_mapping
                }
              >
                {({ getFieldValue }) => {
                  const selectedInterface = interfaceCapabilities.find(
                    (entry) =>
                      entry.interface_id === getFieldValue('interface_id')
                  );

                  return (
                    <div>
                      <McpToolDebugPanel
                        csrfToken={csrfToken}
                        executeDebug={executeSettingsMcpToolDebug}
                        interfaceId={getFieldValue('interface_id')}
                        inputMapping={getFieldValue('input_mapping')}
                        operationLabel={
                          selectedInterface
                            ? interfaceOptionLabel(selectedInterface)
                            : null
                        }
                        outputMapping={schemaRecord(
                          getFieldValue('output_mapping')
                        )}
                      />
                    </div>
                  );
                }}
              </Form.Item>
            )
          ) : null}
        </Form>
      </FixedHeightModal>
    </Space>
  );
}
