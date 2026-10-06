import { useMemo, useState, type ReactNode } from 'react';
import {
  Alert,
  Button,
  ConfigProvider,
  Empty,
  Input,
  Spin,
  Tree,
  Typography,
  theme
} from 'antd';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import TeamOutlined from '@ant-design/icons/es/icons/TeamOutlined';
import type { SettingsDepartment } from '../../api/departments';
import { departmentTree, type DepartmentTreeRow } from './department-tree';
import { i18nText } from '../../../../shared/i18n/text';
import './organization-management.css';
interface OrganizationNode {
  key: string;
  title: ReactNode;
  icon: ReactNode;
  children?: OrganizationNode[];
}
export function OrganizationSelector({
  departments,
  selected,
  onSelect,
  loading,
  error,
  onRetry
}: {
  departments: SettingsDepartment[];
  selected?: string;
  onSelect: (id?: string) => void;
  loading: boolean;
  error: boolean;
  onRetry: () => void;
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
            icon: <FolderOutlined />,
            title: (
              <span className="organization-tree-label">
                <span>{row.name}</span>
                <span className="organization-count">{row.member_count}</span>
              </span>
            ),
            children: children.length ? children : undefined
          }
        ];
      });
    return present(departmentTree(departments));
  }, [departments, search]);
  return (
    <aside className="organization-sidebar">
      <Typography.Title level={4}>
        {i18nText('settings', 'organization.title')}
      </Typography.Title>
      <Input
        prefix={<SearchOutlined />}
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        placeholder={i18nText('settings', 'organization.search')}
        aria-label={i18nText('settings', 'organization.search')}
        allowClear
      />
      <Button
        type="text"
        className={`organization-all${selected ? '' : ' organization-all--selected'}`}
        icon={<TeamOutlined />}
        onClick={() => onSelect()}
      >
        {i18nText('settings', 'organization.all_members')}
      </Button>
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
      ) : departments.length ? (
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
            key={search ? 'search' : 'tree'}
            showIcon
            blockNode
            defaultExpandAll
            selectedKeys={selected ? [selected] : []}
            treeData={treeData}
            onSelect={(keys) => {
              if (keys.length) onSelect(String(keys[0]));
            }}
          />
        </ConfigProvider>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('settings', 'organization.empty')}
        />
      )}
    </aside>
  );
}
