import {
  DeleteOutlined,
  EditOutlined,
  FileOutlined,
  FolderOpenOutlined,
  FolderOutlined
} from '@ant-design/icons';
import { Button, Popconfirm, Tooltip, Tree } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';
import type { McpDirectoryTreeNode } from '../mcp-management-view-model';
import type { McpDirectoryTreeDropInfo } from './directory-tree';

type McpDirectoryTreeProps = {
  canManage: boolean;
  expandedKeys: string[];
  selectedKey: string;
  treeData: McpDirectoryTreeNode[];
  onExpand: (key: string, expanded: boolean) => void;
  onSelect: (key: string) => void;
  onDrop: (info: McpDirectoryTreeDropInfo) => void;
  onEditGroup: (path: string) => void;
  onEditBinding: (bindingId: string) => void;
  onDeleteGroup: (path: string) => void;
  onDeleteBinding: (bindingId: string) => void;
};

export function McpDirectoryTree({
  canManage,
  expandedKeys,
  selectedKey,
  treeData,
  onExpand,
  onSelect,
  onDrop,
  onEditGroup,
  onEditBinding,
  onDeleteGroup,
  onDeleteBinding
}: McpDirectoryTreeProps) {
  return (
    <Tree<McpDirectoryTreeNode>
      className="mcp-management__directory-tree"
      draggable={canManage ? { icon: false } : false}
      blockNode
      expandedKeys={expandedKeys}
      showIcon
      selectedKeys={selectedKey ? [selectedKey] : []}
      treeData={treeData}
      onExpand={(_nextExpandedKeys, info) =>
        onExpand(String(info.node.key), info.expanded)
      }
      onSelect={(selectedKeys) => {
        if (selectedKeys.length > 0) onSelect(String(selectedKeys[0]));
      }}
      onDrop={onDrop}
      titleRender={(node) => {
        const [type, ...parts] = node.key.split(':');
        const isInstance = type === 'instance';
        const isGroup = type === 'group';
        const isBinding = type === 'binding';

        let titleNode: ReactNode = <span>{node.title}</span>;
        if (isGroup) {
          const shortDescription = node.description_short?.trim();
          titleNode = (
            <span className="mcp-management__group-node">
              <span className="mcp-management__group-node-id">
                {node.title}
              </span>
              {shortDescription ? (
                <span className="mcp-management__group-node-description">
                  {shortDescription}
                </span>
              ) : null}
            </span>
          );
        } else if (isBinding) {
          const shortDescription = node.tool_short_description?.trim();
          titleNode = (
            <span className="mcp-management__binding-node">
              <span className="mcp-management__binding-node-id">
                {node.title}
              </span>
              {shortDescription ? (
                <span className="mcp-management__binding-node-description">
                  {shortDescription}
                </span>
              ) : null}
            </span>
          );
        }

        return (
          <span className="mcp-management__tree-node-title">
            {titleNode}
            {canManage && (isInstance || isGroup || isBinding) ? (
              <span
                className={
                  isInstance
                    ? 'mcp-management__tree-node-actions mcp-management__tree-node-actions--visible'
                    : 'mcp-management__tree-node-actions'
                }
                onClick={(event) => event.stopPropagation()}
              >
                {!isInstance ? (
                  <Tooltip title={i18nText('settings', 'auto.edit')}>
                    <Button
                      type="text"
                      size="small"
                      icon={<EditOutlined />}
                      aria-label={i18nText('settings', 'auto.edit')}
                      onClick={() =>
                        isGroup
                          ? onEditGroup(node.path)
                          : onEditBinding(parts.join(':'))
                      }
                    />
                  </Tooltip>
                ) : null}
                {!isInstance ? (
                  <Popconfirm
                    title={
                      isGroup
                        ? i18nText(
                            'settingsMcpManagement',
                            'auto.mcp_group_delete_confirm'
                          )
                        : i18nText('settings', 'auto.mcp_hard_delete_confirm')
                    }
                    onConfirm={() =>
                      isGroup
                        ? onDeleteGroup(parts.join(':'))
                        : onDeleteBinding(parts.join(':'))
                    }
                  >
                    <Button
                      type="text"
                      danger
                      size="small"
                      icon={<DeleteOutlined />}
                      className="ant-btn-dangerous"
                      aria-label="Delete"
                    />
                  </Popconfirm>
                ) : null}
              </span>
            ) : null}
          </span>
        );
      }}
      icon={(nodeProps) => {
        const key = 'key' in nodeProps ? nodeProps.key : undefined;
        if (!key) return null;
        const [type] = String(key).split(':');
        if (type === 'instance') {
          return <FolderOpenOutlined style={{ color: '#1890ff' }} />;
        }
        if (type === 'group') {
          return <FolderOutlined style={{ color: '#faad14' }} />;
        }
        return <FileOutlined style={{ color: '#52c41a' }} />;
      }}
    />
  );
}
