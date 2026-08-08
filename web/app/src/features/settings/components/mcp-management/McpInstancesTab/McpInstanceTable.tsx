import {
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  LinkOutlined,
  MoreOutlined,
  PlusOutlined,
  SearchOutlined,
  SettingOutlined,
  UploadOutlined
} from '@ant-design/icons';
import type { ConsoleMcpInstance } from '@1flowbase/api-client';
import {
  Button,
  Dropdown,
  Flex,
  Modal,
  Space,
  Table,
  Tag,
  Tooltip,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { i18nText } from '../../../../../shared/i18n/text';
import { statusColor } from '../mcp-management-utils';

export function McpInstanceTable({
  canManage,
  instances,
  groupCounts,
  toolCounts,
  onCreate,
  onEdit,
  onOpenDirectory,
  onConnect,
  onEditDiscoveryPolicy,
  onCopy,
  onExport,
  onDelete
}: {
  canManage: boolean;
  instances: ConsoleMcpInstance[];
  groupCounts: Map<string, number>;
  toolCounts: Map<string, number>;
  onCreate: () => void;
  onEdit: (instance: ConsoleMcpInstance) => void;
  onOpenDirectory: (instance: ConsoleMcpInstance) => void;
  onConnect: (instance: ConsoleMcpInstance) => void;
  onEditDiscoveryPolicy: (instance: ConsoleMcpInstance) => void;
  onCopy: (instance: ConsoleMcpInstance) => void;
  onExport: (instance: ConsoleMcpInstance) => void;
  onDelete: (instance: ConsoleMcpInstance) => Promise<unknown>;
}) {
  const columns: ColumnsType<ConsoleMcpInstance> = [
    { title: 'instance_id', dataIndex: 'instance_id' },
    {
      title: i18nText('settings', 'auto.instance_name'),
      dataIndex: 'name',
      render: (name: ConsoleMcpInstance['name']) => (
        <Typography.Text strong>{name}</Typography.Text>
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
      title: i18nText('settingsMcpManagement', 'auto.llm_tool_registration'),
      dataIndex: 'llm_tool_registration',
      render: (registration: ConsoleMcpInstance['llm_tool_registration']) => (
        <Tooltip title={registration.tools.map((tool) => tool.name).join('\n')}>
          <Typography.Text
            code
            copyable={{
              text: registration.tools.map((tool) => tool.name).join('\n')
            }}
          >
            {registration.prefix}
          </Typography.Text>
        </Tooltip>
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      render: (_, record) => (
        <Space>
          <Button
            aria-label={i18nText('settings', 'auto.edit')}
            icon={<EditOutlined />}
            size="small"
            disabled={!canManage}
            onClick={() => onEdit(record)}
          />
          <Tooltip title={i18nText('settings', 'auto.directory_editor')}>
            <Button
              aria-label={i18nText('settings', 'auto.directory_editor')}
              icon={<SettingOutlined />}
              size="small"
              disabled={!canManage}
              onClick={() => onOpenDirectory(record)}
            />
          </Tooltip>
          <Tooltip
            title={i18nText('settingsMcpManagement', 'auto.connect_client')}
          >
            <Button
              aria-label={i18nText(
                'settingsMcpManagement',
                'auto.connect_client'
              )}
              icon={<LinkOutlined />}
              size="small"
              onClick={() => onConnect(record)}
            />
          </Tooltip>
          <Tooltip
            title={i18nText('settingsMcpManagement', 'auto.more_actions')}
          >
            <Dropdown
              trigger={['click']}
              menu={{
                items: [
                  {
                    key: 'discovery_policy',
                    icon: <SearchOutlined />,
                    label: i18nText(
                      'settingsMcpManagement',
                      'auto.discovery_policy'
                    ),
                    disabled: !canManage
                  },
                  {
                    key: 'copy',
                    icon: <CopyOutlined />,
                    label: i18nText(
                      'settingsMcpManagement',
                      'auto.copy_instance'
                    ),
                    disabled: !canManage
                  },
                  {
                    key: 'export',
                    icon: <UploadOutlined />,
                    label: i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_instance_export'
                    ),
                    disabled: !canManage
                  },
                  { type: 'divider' },
                  {
                    key: 'delete',
                    icon: <DeleteOutlined />,
                    label: i18nText('settings', 'auto.delete'),
                    danger: true,
                    disabled: !canManage
                  }
                ],
                onClick: ({ key }) => {
                  if (key === 'discovery_policy') onEditDiscoveryPolicy(record);
                  else if (key === 'export') onExport(record);
                  else if (key === 'copy') onCopy(record);
                  else if (key === 'delete') {
                    Modal.confirm({
                      title: i18nText(
                        'settings',
                        'auto.mcp_hard_delete_confirm'
                      ),
                      okButtonProps: { danger: true },
                      onOk: () => onDelete(record)
                    });
                  }
                }
              }}
            >
              <Button
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.more_actions'
                )}
                icon={<MoreOutlined />}
                size="small"
              />
            </Dropdown>
          </Tooltip>
        </Space>
      )
    }
  ];

  return (
    <>
      <Flex justify="flex-end" align="center">
        <Button
          type="primary"
          icon={<PlusOutlined />}
          disabled={!canManage}
          onClick={onCreate}
        >
          {i18nText('settings', 'auto.new')}
        </Button>
      </Flex>
      <Table
        rowKey="id"
        columns={columns}
        dataSource={instances}
        pagination={false}
      />
    </>
  );
}
