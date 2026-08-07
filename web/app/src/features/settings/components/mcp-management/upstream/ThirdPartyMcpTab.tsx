import {
  DeleteOutlined,
  EditOutlined,
  ExperimentOutlined,
  PlusOutlined,
  SearchOutlined
} from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Checkbox,
  Descriptions,
  Flex,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Spin,
  Table,
  Tag,
  Tooltip,
  Typography,
  App,
  type TableColumnsType
} from 'antd';
import { useMemo, useState } from 'react';
import type {
  ConsoleMcpUpstreamAuthType,
  ConsoleMcpUpstreamConnection,
  ConsoleMcpUpstreamDiscoveredTool,
  SaveConsoleMcpUpstreamConnectionBody,
  SaveConsoleMcpUpstreamConnectionCredentialsBody,
  TestConsoleMcpUpstreamConnectionDraftBody
} from '@1flowbase/api-client';

import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  createSettingsMcpUpstreamConnection,
  deleteSettingsMcpUpstreamConnection,
  deleteSettingsMcpUpstreamConnectionCredentials,
  discoverSettingsMcpUpstreamConnection,
  fetchSettingsMcpUpstreamConnections,
  importSettingsMcpUpstreamTools,
  saveSettingsMcpUpstreamConnectionCredentials,
  settingsMcpCatalogQueryKey,
  settingsMcpUpstreamConnectionsQueryKey,
  testSettingsMcpUpstreamConnection,
  testSettingsMcpUpstreamConnectionDraft,
  updateSettingsMcpUpstreamConnection
} from '../../../api/mcp-management';

type ConnectionFormValues = SaveConsoleMcpUpstreamConnectionBody & {
  token?: string;
  header_name?: string;
  header_value?: string;
};

type ConnectionTestRequest =
  | { kind: 'saved'; connectionId: string }
  | { kind: 'draft'; body: TestConsoleMcpUpstreamConnectionDraftBody };

const textByKey = {
  enabled: () => i18nText('settingsMcpManagement', 'auto.enabled'),
  upstream_actions: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_actions'),
  upstream_auth_type: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_auth_type'),
  upstream_bearer_token_required: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_bearer_token_required'),
  upstream_connection_deleted: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_deleted'),
  upstream_connection_name: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_name'),
  upstream_connection_required: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_required'),
  upstream_connection_saved: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_saved'),
  upstream_connection_status: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_status'),
  upstream_connection_test_success: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_test_success'),
  upstream_connection_test_title: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connection_test_title'),
  upstream_connections_load_failed: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_connections_load_failed'),
  upstream_credentials_configured: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_credentials_configured'),
  upstream_credentials_missing: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_credentials_missing'),
  upstream_credentials_not_required: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_credentials_not_required'),
  upstream_credentials_status: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_credentials_status'),
  upstream_definition_changed: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_definition_changed'),
  upstream_delete_connection: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_delete_connection'),
  upstream_delete_connection_confirm: () =>
    i18nText(
      'settingsMcpManagement',
      'auto.upstream_delete_connection_confirm'
    ),
  upstream_difference_status: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_difference_status'),
  upstream_disabled: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_disabled'),
  upstream_discover_import_title: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_discover_import_title'),
  upstream_discover_tools: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_discover_tools'),
  upstream_discovered_at: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_discovered_at'),
  upstream_edit_connection: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_edit_connection'),
  upstream_edit_connection_title: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_edit_connection_title'),
  upstream_header_name: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_header_name'),
  upstream_header_value: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_header_value'),
  upstream_header_value_required: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_header_value_required'),
  upstream_import_selected: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_import_selected'),
  upstream_imported: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_imported'),
  upstream_input_schema: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_input_schema'),
  upstream_last_connected_at: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_last_connected_at'),
  upstream_last_discovered_at: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_last_discovered_at'),
  upstream_last_error: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_last_error'),
  upstream_new_connection: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_new_connection'),
  upstream_new_connection_title: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_new_connection_title'),
  upstream_no_connections: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_no_connections'),
  upstream_no_fields: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_no_fields'),
  upstream_not_imported: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_not_imported'),
  upstream_output_schema: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_output_schema'),
  upstream_protocol_version: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_protocol_version'),
  upstream_remote_missing: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_remote_missing'),
  upstream_remote_tool_name: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_remote_tool_name'),
  upstream_search_tools: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_search_tools'),
  upstream_select_tool: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_select_tool'),
  upstream_server_name: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_server_name'),
  upstream_server_version: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_server_version'),
  upstream_test: () => i18nText('settingsMcpManagement', 'auto.upstream_test'),
  upstream_test_connection: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_test_connection'),
  upstream_tested_at: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_tested_at'),
  upstream_tool_description: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_tool_description'),
  upstream_tools_imported: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_tools_imported'),
  upstream_unknown_status: () =>
    i18nText('settingsMcpManagement', 'auto.upstream_unknown_status')
};

function text(key: keyof typeof textByKey) {
  return textByKey[key]();
}

function sourceStatusLabel(status: string) {
  const labels: Record<string, string> = {
    not_imported: text('upstream_not_imported'),
    imported: text('upstream_imported'),
    definition_changed: text('upstream_definition_changed'),
    remote_missing: text('upstream_remote_missing')
  };
  return labels[status] ?? text('upstream_unknown_status');
}

function sourceStatusColor(status: string) {
  if (status === 'imported') return 'green';
  if (status === 'definition_changed') return 'orange';
  if (status === 'remote_missing') return 'red';
  return 'blue';
}

function connectionStatusLabel(status: string) {
  if (status === 'enabled') return text('enabled');
  if (status === 'disabled') return text('upstream_disabled');
  return text('upstream_unknown_status');
}

function credentialsStatusLabel(status: string) {
  if (status === 'configured' || status === 'saved') {
    return text('upstream_credentials_configured');
  }
  if (status === 'missing') return text('upstream_credentials_missing');
  if (status === 'not_required')
    return text('upstream_credentials_not_required');
  return text('upstream_unknown_status');
}

function formatTimestamp(value: string | null) {
  return value ? new Date(value).toLocaleString() : '—';
}

function schemaFieldRows(schema: unknown, prefix = ''): string[] {
  if (typeof schema !== 'object' || schema === null || Array.isArray(schema)) {
    return [];
  }
  const properties = (schema as Record<string, unknown>).properties;
  if (
    typeof properties !== 'object' ||
    properties === null ||
    Array.isArray(properties)
  ) {
    return [];
  }

  return Object.entries(properties as Record<string, unknown>).flatMap(
    ([name, child]) => {
      const path = prefix ? `${prefix}.${name}` : name;
      const nested = schemaFieldRows(child, path);
      return nested.length > 0 ? [path, ...nested] : [path];
    }
  );
}

function SchemaFields({ schema }: { schema: unknown }) {
  const fields = schemaFieldRows(schema);
  if (fields.length === 0) {
    return (
      <Typography.Text type="secondary">
        {text('upstream_no_fields')}
      </Typography.Text>
    );
  }
  return (
    <Space size={[4, 4]} wrap>
      {fields.map((field) => (
        <Tag key={field}>{field}</Tag>
      ))}
    </Space>
  );
}

export function ThirdPartyMcpTab({
  canManage,
  onImported
}: {
  canManage: boolean;
  onImported: () => void;
}) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const [form] = Form.useForm<ConnectionFormValues>();
  const authType = Form.useWatch('auth_type', form);
  const [connectionModalOpen, setConnectionModalOpen] = useState(false);
  const [editingConnection, setEditingConnection] =
    useState<ConsoleMcpUpstreamConnection | null>(null);
  const [testResultOpen, setTestResultOpen] = useState(false);
  const [discoverConnection, setDiscoverConnection] =
    useState<ConsoleMcpUpstreamConnection | null>(null);
  const [toolKeyword, setToolKeyword] = useState('');
  const [selectedToolNames, setSelectedToolNames] = useState<string[]>([]);

  const connectionsQuery = useQuery({
    queryKey: settingsMcpUpstreamConnectionsQueryKey,
    queryFn: fetchSettingsMcpUpstreamConnections
  });

  const validateConnectionCredential = (values: ConnectionFormValues) => {
    if (
      values.auth_type === 'bearer' &&
      editingConnection?.auth_type !== 'bearer' &&
      !values.token
    ) {
      throw new Error(text('upstream_bearer_token_required'));
    }
    if (
      values.auth_type === 'custom_header' &&
      (editingConnection?.auth_type !== 'custom_header' ||
        editingConnection.custom_header_name !== values.header_name) &&
      !values.header_value
    ) {
      throw new Error(text('upstream_header_value_required'));
    }
  };

  const saveMutation = useMutation({
    mutationFn: async (values: ConnectionFormValues) => {
      validateConnectionCredential(values);
      const body: SaveConsoleMcpUpstreamConnectionBody = {
        name: values.name,
        endpoint: values.endpoint,
        transport: values.transport,
        auth_type: values.auth_type,
        custom_header_name:
          values.auth_type === 'custom_header'
            ? (values.header_name ?? null)
            : null,
        status: values.status
      };
      const saved = editingConnection
        ? await updateSettingsMcpUpstreamConnection(
            editingConnection.connection_id,
            body,
            csrfToken
          )
        : await createSettingsMcpUpstreamConnection(body, csrfToken);
      if (!editingConnection) {
        setEditingConnection(saved);
      }

      if (values.auth_type === 'bearer' && values.token) {
        await saveSettingsMcpUpstreamConnectionCredentials(
          saved.connection_id,
          { kind: 'bearer', token: values.token },
          csrfToken
        );
      } else if (
        values.auth_type === 'custom_header' &&
        values.header_name &&
        values.header_value
      ) {
        await saveSettingsMcpUpstreamConnectionCredentials(
          saved.connection_id,
          {
            kind: 'custom_header',
            header_name: values.header_name,
            header_value: values.header_value
          },
          csrfToken
        );
      } else if (
        editingConnection &&
        values.auth_type === 'none' &&
        editingConnection.auth_type !== 'none'
      ) {
        await deleteSettingsMcpUpstreamConnectionCredentials(
          saved.connection_id,
          csrfToken
        );
      }
    },
    onSuccess: async () => {
      message.success(text('upstream_connection_saved'));
      setConnectionModalOpen(false);
      setEditingConnection(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpUpstreamConnectionsQueryKey
      });
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) =>
      deleteSettingsMcpUpstreamConnection(connectionId, csrfToken),
    onSuccess: async () => {
      message.success(text('upstream_connection_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpUpstreamConnectionsQueryKey
      });
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });

  const testMutation = useMutation({
    mutationFn: (request: ConnectionTestRequest) =>
      request.kind === 'saved'
        ? testSettingsMcpUpstreamConnection(request.connectionId, csrfToken)
        : testSettingsMcpUpstreamConnectionDraft(request.body, csrfToken),
    onSuccess: async (_result, request) => {
      if (request.kind === 'saved') {
        await queryClient.invalidateQueries({
          queryKey: settingsMcpUpstreamConnectionsQueryKey
        });
      }
    }
  });

  const discoverMutation = useMutation({
    mutationFn: (connectionId: string) =>
      discoverSettingsMcpUpstreamConnection(connectionId, csrfToken),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsMcpUpstreamConnectionsQueryKey
      });
    }
  });

  const importMutation = useMutation({
    mutationFn: () => {
      if (!discoverConnection) {
        throw new Error(text('upstream_connection_required'));
      }
      return importSettingsMcpUpstreamTools(
        discoverConnection.connection_id,
        { remote_tool_names: selectedToolNames },
        csrfToken
      );
    },
    onSuccess: async () => {
      message.success(text('upstream_tools_imported'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
      setDiscoverConnection(null);
      setSelectedToolNames([]);
      onImported();
    },
    onError: (error) => {
      message.error(error instanceof Error ? error.message : String(error));
    }
  });

  const testCurrentForm = async () => {
    try {
      const values = await form.validateFields();
      validateConnectionCredential(values);
      let credential: SaveConsoleMcpUpstreamConnectionCredentialsBody | null =
        null;
      if (values.auth_type === 'bearer' && values.token) {
        credential = { kind: 'bearer', token: values.token };
      } else if (
        values.auth_type === 'custom_header' &&
        values.header_name &&
        values.header_value
      ) {
        credential = {
          kind: 'custom_header',
          header_name: values.header_name,
          header_value: values.header_value
        };
      }
      testMutation.reset();
      setTestResultOpen(true);
      testMutation.mutate({
        kind: 'draft',
        body: {
          connection_id: editingConnection?.connection_id ?? null,
          endpoint: values.endpoint,
          transport: values.transport,
          auth_type: values.auth_type,
          custom_header_name:
            values.auth_type === 'custom_header'
              ? (values.header_name ?? null)
              : null,
          credential
        }
      });
    } catch (error) {
      if (error instanceof Error) {
        message.error(error.message);
      }
    }
  };

  const columns = useMemo<TableColumnsType<ConsoleMcpUpstreamConnection>>(
    () => [
      {
        title: text('upstream_connection_name'),
        dataIndex: 'name',
        key: 'name',
        width: 180
      },
      {
        title: 'Endpoint',
        dataIndex: 'endpoint',
        key: 'endpoint',
        ellipsis: true
      },
      {
        title: text('upstream_auth_type'),
        dataIndex: 'auth_type',
        key: 'auth_type',
        width: 150
      },
      {
        title: text('upstream_credentials_status'),
        dataIndex: 'credentials_status',
        key: 'credentials_status',
        width: 150,
        render: (value) => credentialsStatusLabel(String(value))
      },
      {
        title: text('upstream_connection_status'),
        dataIndex: 'status',
        key: 'status',
        width: 110,
        render: (value) => (
          <Tag color={value === 'enabled' ? 'green' : 'default'}>
            {connectionStatusLabel(String(value))}
          </Tag>
        )
      },
      {
        title: text('upstream_last_connected_at'),
        dataIndex: 'last_connected_at',
        key: 'last_connected_at',
        width: 180,
        render: (value) => formatTimestamp(value as string | null)
      },
      {
        title: text('upstream_last_discovered_at'),
        dataIndex: 'last_discovered_at',
        key: 'last_discovered_at',
        width: 180,
        render: (value) => formatTimestamp(value as string | null)
      },
      {
        title: text('upstream_last_error'),
        dataIndex: 'last_error',
        key: 'last_error',
        width: 220,
        ellipsis: true,
        render: (value) =>
          value ? (
            <Typography.Text type="danger">{String(value)}</Typography.Text>
          ) : (
            '—'
          )
      },
      {
        title: text('upstream_actions'),
        key: 'actions',
        width: 190,
        fixed: 'right',
        render: (_, record) => (
          <Space size="small">
            <Tooltip title={text('upstream_edit_connection')}>
              <Button
                aria-label={`${text('upstream_edit_connection')} ${record.name}`}
                icon={<EditOutlined />}
                size="small"
                disabled={!canManage}
                onClick={() => {
                  setEditingConnection(record);
                  form.setFieldsValue({
                    name: record.name,
                    endpoint: record.endpoint,
                    transport: record.transport,
                    auth_type: record.auth_type,
                    status: record.status,
                    header_name: record.custom_header_name ?? undefined,
                    token: undefined,
                    header_value: undefined
                  });
                  setConnectionModalOpen(true);
                }}
              />
            </Tooltip>
            <Tooltip title={text('upstream_test_connection')}>
              <Button
                aria-label={`${text('upstream_test_connection')} ${record.name}`}
                icon={<ExperimentOutlined />}
                size="small"
                disabled={!canManage}
                onClick={() => {
                  setTestResultOpen(true);
                  testMutation.reset();
                  testMutation.mutate({
                    kind: 'saved',
                    connectionId: record.connection_id
                  });
                }}
              />
            </Tooltip>
            <Tooltip title={text('upstream_discover_tools')}>
              <Button
                aria-label={`${text('upstream_discover_tools')} ${record.name}`}
                icon={<SearchOutlined />}
                size="small"
                disabled={!canManage}
                onClick={() => {
                  setDiscoverConnection(record);
                  setToolKeyword('');
                  setSelectedToolNames([]);
                  discoverMutation.reset();
                  discoverMutation.mutate(record.connection_id);
                }}
              />
            </Tooltip>
            <Popconfirm
              title={text('upstream_delete_connection_confirm')}
              onConfirm={() => deleteMutation.mutate(record.connection_id)}
            >
              <Button
                aria-label={`${text('upstream_delete_connection')} ${record.name}`}
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
    [canManage, deleteMutation, discoverMutation, form, testMutation]
  );

  const discoveredTools = discoverMutation.data?.items ?? [];
  const filteredTools = discoveredTools.filter((tool) => {
    const keyword = toolKeyword.trim().toLowerCase();
    return (
      keyword.length === 0 ||
      tool.remote_tool_name.toLowerCase().includes(keyword) ||
      tool.description?.toLowerCase().includes(keyword)
    );
  });

  const discoveryColumns: TableColumnsType<ConsoleMcpUpstreamDiscoveredTool> = [
    {
      title: '',
      key: 'selection',
      width: 48,
      render: (_, record) => (
        <Checkbox
          aria-label={`${text('upstream_select_tool')} ${record.remote_tool_name}`}
          checked={selectedToolNames.includes(record.remote_tool_name)}
          disabled={record.source_status === 'remote_missing'}
          onChange={(event) => {
            setSelectedToolNames((current) =>
              event.target.checked
                ? [...current, record.remote_tool_name]
                : current.filter((name) => name !== record.remote_tool_name)
            );
          }}
        />
      )
    },
    {
      title: text('upstream_remote_tool_name'),
      dataIndex: 'remote_tool_name',
      key: 'remote_tool_name',
      width: 220
    },
    {
      title: text('upstream_tool_description'),
      dataIndex: 'description',
      key: 'description',
      ellipsis: true,
      render: (value) => value || '—'
    },
    {
      title: text('upstream_difference_status'),
      dataIndex: 'source_status',
      key: 'source_status',
      width: 140,
      render: (value) => (
        <Tag color={sourceStatusColor(String(value))}>
          {sourceStatusLabel(String(value))}
        </Tag>
      )
    }
  ];

  if (connectionsQuery.isLoading) {
    return <Spin />;
  }
  if (connectionsQuery.isError || !connectionsQuery.data) {
    return (
      <Alert type="error" title={text('upstream_connections_load_failed')} />
    );
  }

  return (
    <Space
      orientation="vertical"
      size="middle"
      className="mcp-management__stack"
    >
      <Flex justify="flex-end">
        <Button
          aria-label={text('upstream_new_connection')}
          type="primary"
          icon={<PlusOutlined />}
          disabled={!canManage}
          onClick={() => {
            setEditingConnection(null);
            form.resetFields();
            form.setFieldsValue({
              transport: 'streamable_http',
              auth_type: 'none',
              status: 'enabled'
            });
            setConnectionModalOpen(true);
          }}
        >
          {text('upstream_new_connection')}
        </Button>
      </Flex>

      <Table
        columns={columns}
        dataSource={connectionsQuery.data}
        pagination={false}
        rowKey="connection_id"
        scroll={{ x: 1560 }}
        locale={{ emptyText: text('upstream_no_connections') }}
      />

      <Modal
        open={connectionModalOpen}
        title={
          editingConnection
            ? text('upstream_edit_connection_title')
            : text('upstream_new_connection_title')
        }
        footer={[
          <Button
            key="cancel"
            aria-label={i18nText('settings', 'auto.cancel')}
            disabled={saveMutation.isPending || testMutation.isPending}
            onClick={() => setConnectionModalOpen(false)}
          >
            {i18nText('settings', 'auto.cancel')}
          </Button>,
          <Button
            key="test"
            aria-label={text('upstream_test')}
            disabled={!canManage || saveMutation.isPending}
            loading={testMutation.isPending}
            onClick={() => void testCurrentForm()}
          >
            {text('upstream_test')}
          </Button>,
          <Button
            key="save"
            aria-label={i18nText('settings', 'auto.save')}
            type="primary"
            disabled={!canManage || testMutation.isPending}
            loading={saveMutation.isPending}
            onClick={() => form.submit()}
          >
            {i18nText('settings', 'auto.save')}
          </Button>
        ]}
        onCancel={() => setConnectionModalOpen(false)}
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={(values) => saveMutation.mutate(values)}
        >
          <Form.Item
            name="name"
            label={text('upstream_connection_name')}
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="endpoint"
            label="Endpoint"
            rules={[{ required: true }, { type: 'url' }]}
          >
            <Input placeholder="https://mcp.example.com/mcp" />
          </Form.Item>
          <Form.Item
            name="transport"
            label="Transport"
            rules={[{ required: true }]}
          >
            <Select
              virtual={false}
              options={[
                {
                  value: 'streamable_http',
                  label: 'HTTPS Streamable HTTP'
                }
              ]}
            />
          </Form.Item>
          <Form.Item
            name="auth_type"
            label={text('upstream_auth_type')}
            rules={[{ required: true }]}
          >
            <Select
              virtual={false}
              options={(
                [
                  'none',
                  'bearer',
                  'custom_header'
                ] as ConsoleMcpUpstreamAuthType[]
              ).map((value) => ({ value, label: value }))}
            />
          </Form.Item>
          {authType === 'bearer' ? (
            <Form.Item
              name="token"
              label="Bearer token"
              rules={[
                {
                  required:
                    !editingConnection ||
                    editingConnection.auth_type !== 'bearer'
                }
              ]}
            >
              <Input.Password autoComplete="new-password" />
            </Form.Item>
          ) : null}
          {authType === 'custom_header' ? (
            <>
              <Form.Item
                name="header_name"
                label={text('upstream_header_name')}
                rules={[{ required: true }]}
              >
                <Input />
              </Form.Item>
              <Form.Item
                name="header_value"
                label={text('upstream_header_value')}
                rules={[
                  {
                    required:
                      !editingConnection ||
                      editingConnection.auth_type !== 'custom_header'
                  }
                ]}
              >
                <Input.Password autoComplete="new-password" />
              </Form.Item>
            </>
          ) : null}
          <Form.Item
            name="status"
            label={text('upstream_connection_status')}
            rules={[{ required: true }]}
          >
            <Select
              options={['enabled', 'disabled'].map((value) => ({
                value,
                label: connectionStatusLabel(value)
              }))}
            />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        open={testResultOpen}
        title={text('upstream_connection_test_title')}
        footer={null}
        onCancel={() => setTestResultOpen(false)}
        destroyOnHidden
      >
        {testMutation.isPending ? <Spin /> : null}
        {testMutation.isError ? (
          <Alert
            type="error"
            title={
              testMutation.error instanceof Error
                ? testMutation.error.message
                : String(testMutation.error)
            }
          />
        ) : null}
        {testMutation.data ? (
          <Space
            orientation="vertical"
            size="middle"
            className="mcp-management__stack"
          >
            <Alert
              type={testMutation.data.ok ? 'success' : 'error'}
              title={
                testMutation.data.ok
                  ? text('upstream_connection_test_success')
                  : testMutation.data.error
              }
            />
            <Descriptions column={1} size="small">
              <Descriptions.Item label={text('upstream_server_name')}>
                {testMutation.data.server_name ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_server_version')}>
                {testMutation.data.server_version ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_protocol_version')}>
                {testMutation.data.protocol_version ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_tested_at')}>
                {formatTimestamp(testMutation.data.tested_at)}
              </Descriptions.Item>
            </Descriptions>
          </Space>
        ) : null}
      </Modal>

      <Modal
        width={920}
        open={discoverConnection !== null}
        title={text('upstream_discover_import_title')}
        okText={text('upstream_import_selected')}
        cancelText={i18nText('settings', 'auto.cancel')}
        okButtonProps={{ disabled: selectedToolNames.length === 0 }}
        confirmLoading={importMutation.isPending}
        onOk={() => importMutation.mutate()}
        onCancel={() => setDiscoverConnection(null)}
        destroyOnHidden
      >
        {discoverMutation.isPending ? <Spin /> : null}
        {discoverMutation.isError ? (
          <Alert
            type="error"
            title={
              discoverMutation.error instanceof Error
                ? discoverMutation.error.message
                : String(discoverMutation.error)
            }
          />
        ) : null}
        {discoverMutation.data ? (
          <Space
            orientation="vertical"
            size="middle"
            className="mcp-management__stack"
          >
            <Descriptions size="small" column={{ xs: 1, sm: 3 }}>
              <Descriptions.Item label={text('upstream_server_name')}>
                {discoverMutation.data.server_name ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_server_version')}>
                {discoverMutation.data.server_version ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_protocol_version')}>
                {discoverMutation.data.protocol_version}
              </Descriptions.Item>
              <Descriptions.Item label={text('upstream_discovered_at')}>
                {formatTimestamp(discoverMutation.data.discovered_at)}
              </Descriptions.Item>
            </Descriptions>
            <Input.Search
              allowClear
              placeholder={text('upstream_search_tools')}
              value={toolKeyword}
              onChange={(event) => setToolKeyword(event.target.value)}
            />
            <Table
              columns={discoveryColumns}
              dataSource={filteredTools}
              pagination={false}
              rowKey="remote_tool_name"
              scroll={{ x: 760, y: 360 }}
              expandable={{
                expandedRowRender: (record) => (
                  <Descriptions size="small" column={1}>
                    <Descriptions.Item label={text('upstream_input_schema')}>
                      <SchemaFields schema={record.input_schema} />
                    </Descriptions.Item>
                    <Descriptions.Item label={text('upstream_output_schema')}>
                      <SchemaFields schema={record.output_schema} />
                    </Descriptions.Item>
                  </Descriptions>
                )
              }}
            />
          </Space>
        ) : null}
      </Modal>
    </Space>
  );
}
