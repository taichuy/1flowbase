import { useMemo, useState, type ReactNode } from 'react';
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
import type { SettingsDepartment } from '../../api/departments';
import { departmentTree, type DepartmentTreeRow } from './department-tree';
import { i18nText } from '../../../../shared/i18n/text';
import './organization-management.css';
interface OrganizationNode {
  key: string;
  title: ReactNode;
  children?: OrganizationNode[];
}
const ORGANIZATION_ROOT_KEY = 'organization-browse-root';
export function OrganizationSelector({
  departments,
  selected,
  onSelect,
  loading,
  error,
  onRetry,
  onCreate
}: {
  departments: SettingsDepartment[];
  selected?: string;
  onSelect: (id?: string) => void;
  loading: boolean;
  error: boolean;
  onRetry: () => void;
  onCreate?: () => void;
}) {
  const [search, setSearch] = useState('');
  const { token } = theme.useToken();
  const treeData = useMemo(() => {
    const present = (rows: DepartmentTreeRow[]): OrganizationNode[] =>
      rows.flatMap((row) => {
        const children = present(row.children ?? []);
        if (
          search &&
          !row.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()) &&
          !children.length
        )
          return [];
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
            children: children.length ? children : undefined
          }
        ];
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
        children: present(departmentTree(departments))
      }
    ];
  }, [departments, search]);
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
        onChange={(event) => setSearch(event.target.value)}
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
          key={`${search ? 'search' : 'tree'}:${departments.map((department) => department.id).join(',')}`}
          blockNode
          defaultExpandAll
          selectedKeys={[selected ?? ORGANIZATION_ROOT_KEY]}
          treeData={treeData}
          onSelect={(keys) => {
            if (keys.length)
              onSelect(
                keys[0] === ORGANIZATION_ROOT_KEY ? undefined : String(keys[0])
              );
          }}
        />
      </ConfigProvider>
    </aside>
  );
}
