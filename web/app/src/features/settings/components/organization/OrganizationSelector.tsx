import { useMemo, type ReactNode } from 'react';
import {
  Alert,
  Button,
  ConfigProvider,
  Input,
  Spin,
  Tree,
  Typography,
  theme
} from 'antd';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import TeamOutlined from '@ant-design/icons/es/icons/TeamOutlined';
import type { DepartmentTreeItem } from '../../api/departments';
import { departmentTree, type DepartmentTreeRow } from './department-tree';
import { i18nText } from '../../../../shared/i18n/text';
import './organization-management.css';
interface OrganizationNode {
  key: string;
  title: ReactNode;
  children?: OrganizationNode[];
  isLeaf?: boolean;
}
const ORGANIZATION_ROOT_KEY = 'organization-browse-root';
export function OrganizationSelector({
  departments,
  selected,
  onSelect,
  loading,
  error,
  onRetry,
  onCreate,
  search,
  onSearch,
  onExpand,
  groups
}: {
  departments: DepartmentTreeItem[];
  search: string;
  onSearch: (value: string) => void;
  onExpand: (id: string) => Promise<void>;
  groups: { parent_id?: string; loadMore: () => void }[];
  selected?: string;
  onSelect: (id?: string) => void;
  loading: boolean;
  error: boolean;
  onRetry: () => void;
  onCreate?: () => void;
}) {
  const searching = Boolean(search.trim());
  const { token } = theme.useToken();
  const treeData = useMemo(() => {
    const present = (rows: DepartmentTreeRow[]): OrganizationNode[] =>
      rows.flatMap((row) => {
        const children = present(row.children ?? []);
        const group = groups.find((group) => group.parent_id === row.id);
        if (group)
          children.push({
            key: `more:${row.id}`,
            title: (
              <Button
                size="small"
                onClick={(event) => {
                  event.stopPropagation();
                  group.loadMore();
                }}
              >
                {i18nText('settings', 'organization.load_more')}
              </Button>
            ),
            isLeaf: true
          });
        return [
          {
            key: row.id,
            title: (
              <span className="organization-tree-label">
                <FolderOutlined className="organization-tree-folder" />
                <span className="organization-tree-name">{row.name}</span>
                <span className="organization-count">{row.member_count}</span>
              </span>
            ),
            isLeaf: !row.has_children,
            children: children.length ? children : undefined
          }
        ];
      });
    const roots = present(departmentTree(departments));
    const rootGroup = groups.find((group) => !group.parent_id);
    if (rootGroup)
      roots.push({
        key: 'more:root',
        title: (
          <Button
            size="small"
            onClick={(event) => {
              event.stopPropagation();
              rootGroup.loadMore();
            }}
          >
            {i18nText('settings', 'organization.load_more')}
          </Button>
        ),
        isLeaf: true
      });
    return [
      {
        key: ORGANIZATION_ROOT_KEY,
        title: (
          <span className="organization-tree-label">
            <TeamOutlined />
            <span className="organization-tree-name">
              {i18nText('settings', 'organization.title')}
            </span>
          </span>
        ),
        children: roots
      }
    ];
  }, [departments, search, groups]);
  return (
    <aside className="organization-sidebar">
      <div className="organization-toolbar">
        <Typography.Title level={4}>
          {i18nText('settings', 'organization.title')}
        </Typography.Title>
        {onCreate ? (
          <Button
            className="organization-create-button"
            icon={<PlusOutlined />}
            onClick={onCreate}
          >
            {i18nText('settings', 'organization.create')}
          </Button>
        ) : null}
      </div>
      <Input
        prefix={<SearchOutlined />}
        value={search}
        onChange={(event) => onSearch(event.target.value)}
        placeholder={i18nText('settings', 'organization.search')}
        aria-label={i18nText('settings', 'organization.search')}
        allowClear
      />
      {loading ? (
        <Spin />
      ) : error ? (
        <Alert
          type="error"
          message={i18nText('settings', 'organization.load_error')}
          action={
            <Button onClick={onRetry}>
              {i18nText('settings', 'auto.retry_permission_data')}
            </Button>
          }
        />
      ) : null}
      <ConfigProvider
        theme={{
          components: {
            Tree: {
              nodeSelectedBg: token.colorPrimaryBg,
              nodeSelectedColor: token.colorPrimaryText
            }
          }
        }}
      >
        <Tree
          key={searching ? `search:${search.trim()}` : 'tree'}
          blockNode
          defaultExpandedKeys={
            searching
              ? departments.map((item) => item.id).concat(ORGANIZATION_ROOT_KEY)
              : [ORGANIZATION_ROOT_KEY]
          }
          expandedKeys={
            searching
              ? departments.map((item) => item.id).concat(ORGANIZATION_ROOT_KEY)
              : undefined
          }
          onExpand={(_, { expanded, node }) => {
            if (expanded && !searching && node.key !== ORGANIZATION_ROOT_KEY)
              void onExpand(String(node.key));
          }}
          selectedKeys={[selected ?? ORGANIZATION_ROOT_KEY]}
          treeData={treeData}
          onSelect={(keys) => {
            if (keys.length && !String(keys[0]).startsWith('more:'))
              onSelect(
                keys[0] === ORGANIZATION_ROOT_KEY ? undefined : String(keys[0])
              );
          }}
        />
      </ConfigProvider>
    </aside>
  );
}
