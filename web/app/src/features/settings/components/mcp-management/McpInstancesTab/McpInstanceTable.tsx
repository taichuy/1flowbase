import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import LinkOutlined from '@ant-design/icons/es/icons/LinkOutlined';
import MoreOutlined from '@ant-design/icons/es/icons/MoreOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import SettingOutlined from '@ant-design/icons/es/icons/SettingOutlined';
import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import type { ConsoleMcpInstance } from '@1flowbase/api-client';
import {
  Button,
  Dropdown,
  Flex,
  Modal,
  Space,
  Tag,
  Tooltip,
  Typography
} from 'antd';
import { useMemo, useState } from 'react';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { DataTableLayout } from '../../../../../shared/ui/data-table/DataTableLayout';
import { useUserPreferenceDataTableConfiguration } from '../../../../../shared/ui/data-table/user-preference-data-table';

import { i18nText } from '../../../../../shared/i18n/text';
import { statusColor } from '../mcp-management-utils';

export function McpInstanceTable({
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
  onRestoreDefault,
  onDelete
}: {
  canManage?: boolean;
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
  onRestoreDefault: (instance: ConsoleMcpInstance) => void;
  onDelete: (instance: ConsoleMcpInstance) => Promise<unknown>;
}) {
  const can = useConsoleOperationAccess();
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const currentPage = Math.min(
    page,
    Math.max(1, Math.ceil(instances.length / pageSize))
  );
  const columns = useMemo<Array<DataTableColumn<ConsoleMcpInstance>>>(
    () => [
      {
        key: 'instance_id',
        title: 'instance_id',
        dataIndex: 'instance_id',
        width: 200
      },
      {
        key: 'name',
        width: 200,
        title: i18nText('settings', 'auto.instance_name'),
        dataIndex: 'name',
        render: (name) => (
          <Typography.Text strong>{String(name)}</Typography.Text>
        )
      },
      {
        key: 'description_short',
        width: 360,
        sizing: 'fill',
        ellipsis: true,
        title: i18nText('settingsMcpManagement', 'auto.instance_description'),
        dataIndex: 'description_short',
        render: (description) => (
          <Typography.Text type={description ? undefined : 'secondary'}>
            {description ? String(description) : '-'}
          </Typography.Text>
        )
      },
      {
        key: 'status',
        width: 110,
        title: i18nText('settings', 'auto.status'),
        dataIndex: 'status',
        render: (status) => (
          <Tag color={statusColor(String(status))}>{String(status)}</Tag>
        )
      },
      {
        key: 'managed_by',
        width: 240,
        title: i18nText('settingsMcpManagement', 'auto.bundle_source'),
        render: (_, record) =>
          record.managed_by ? (
            <Tag color="blue">
              {record.managed_by.organization}/{record.managed_by.bundle_id}@
              {record.managed_by.bundle_version}
            </Tag>
          ) : (
            '-'
          )
      },
      {
        key: 'directory_summary',
        width: 130,
        title: i18nText('settings', 'auto.directory_summary'),
        render: (_, record) => (
          <Typography.Text>
            {groupCounts.get(record.id) ?? 0} / {toolCounts.get(record.id) ?? 0}
          </Typography.Text>
        )
      },
      {
        key: 'actions',
        width: 180,
        title: i18nText('settings', 'auto.operation'),
        render: (_, record) => (
          <Space>
            <Button
              aria-label={i18nText(
                'settings',
                can('mcp.instances.update') ? 'auto.edit' : 'auto.view'
              )}
              icon={<EditOutlined />}
              size="small"
              onClick={() => onEdit(record)}
            />
            <Tooltip title={i18nText('settings', 'auto.directory_editor')}>
              <Button
                aria-label={i18nText('settings', 'auto.directory_editor')}
                disabled={!can('mcp.directory.view')}
                icon={<SettingOutlined />}
                size="small"
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
                      disabled: !can('mcp.discovery_policy.view')
                    },
                    {
                      key: 'copy',
                      icon: <CopyOutlined />,
                      label: i18nText(
                        'settingsMcpManagement',
                        'auto.copy_instance'
                      ),
                      disabled: !can('mcp.instances.copy')
                    },
                    {
                      key: 'export',
                      icon: <UploadOutlined />,
                      label: i18nText(
                        'settingsMcpManagement',
                        'auto.mcp_instance_export'
                      ),
                      disabled: !can('mcp.instances.export')
                    },
                    ...(record.managed_by?.organization === '1flowbase' &&
                    record.managed_by.bundle_id === 'frontstage_assistant'
                      ? [
                          {
                            key: 'restore_default',
                            icon: <ReloadOutlined />,
                            label: i18nText(
                              'settingsMcpManagement',
                              'auto.restore_instance_default'
                            ),
                            disabled: !can('import_official_mcp_bundle')
                          }
                        ]
                      : []),
                    { type: 'divider' },
                    {
                      key: 'delete',
                      icon: <DeleteOutlined />,
                      label: i18nText('settings', 'auto.delete'),
                      danger: true,
                      disabled: !can('mcp.instances.delete')
                    }
                  ],
                  onClick: ({ key }) => {
                    if (key === 'discovery_policy')
                      onEditDiscoveryPolicy(record);
                    else if (key === 'export') onExport(record);
                    else if (key === 'copy') onCopy(record);
                    else if (key === 'restore_default') {
                      onRestoreDefault(record);
                    } else if (key === 'delete') {
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
    ],
    [
      can,
      groupCounts,
      toolCounts,
      onEdit,
      onOpenDirectory,
      onConnect,
      onEditDiscoveryPolicy,
      onCopy,
      onExport,
      onRestoreDefault,
      onDelete
    ]
  );
  const configuration = useUserPreferenceDataTableConfiguration({
    columns,
    preferenceKey: 'settings.mcp-management.instances.v1'
  });

  return (
    <DataTableLayout>
      <DataTable
        rowKey="id"
        columns={columns}
        configuration={configuration}
        dataSource={instances.slice(
          (currentPage - 1) * pageSize,
          currentPage * pageSize
        )}
        page={currentPage}
        pageSize={pageSize}
        pageSizeOptions={[10, 20, 50, 100]}
        total={instances.length}
        onPageChange={(nextPage, nextSize) => {
          setPage(nextSize === pageSize ? nextPage : 1);
          setPageSize(nextSize);
        }}
        toolbar={
          <Flex justify="flex-end" align="center" gap={8} wrap>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              disabled={!can('mcp.instances.create')}
              onClick={onCreate}
            >
              {i18nText('settings', 'auto.new')}
            </Button>
            <DataTableColumnSettings
              columns={columns}
              configuration={configuration}
            />
          </Flex>
        }
      />
    </DataTableLayout>
  );
}
