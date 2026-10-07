import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Empty,
  Popconfirm,
  Space,
  Table,
  Typography
} from 'antd';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  deleteSettingsDepartment,
  fetchSettingsDepartments,
  settingsDepartmentsQueryKey,
  type DepartmentAccess,
  type SettingsDepartment
} from '../../api/departments';
export type { DepartmentAccess } from '../../api/departments';
import { settingsMembersQueryKey } from '../../api/members';
import { departmentTree, type DepartmentTreeRow } from './department-tree';
import {
  DepartmentEditorDrawer,
  type DepartmentDraft
} from './DepartmentEditorDrawer';
import './organization-management.css';
export function DepartmentManagementPanel({
  access
}: {
  access: DepartmentAccess;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const client = useQueryClient();
  const [draft, setDraft] = useState<DepartmentDraft>();
  const query = useQuery({
    queryKey: settingsDepartmentsQueryKey,
    queryFn: fetchSettingsDepartments,
    enabled: access.can_list
  });
  const data = query.data ?? [];
  const tree = useMemo(() => departmentTree(data), [data]);
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: settingsDepartmentsQueryKey }),
      client.invalidateQueries({ queryKey: settingsMembersQueryKey })
    ]);
  };
  const remove = useMutation({
    mutationFn: (id: string) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return deleteSettingsDepartment(id, csrfToken);
    },
    onSuccess: refresh
  });
  const open = (
    department?: SettingsDepartment,
    parent_id: string | null = null
  ) => {
    setDraft({ department, parent_id });
  };
  if (!access.can_list)
    return (
      <Empty description={i18nText('settings', 'organization.no_access')} />
    );
  return (
    <section className="organization-management">
      <div className="organization-toolbar">
        <Typography.Title level={4}>
          {i18nText('settings', 'organization.management')}
        </Typography.Title>
        {access.can_create ? (
          <Button type="primary" icon={<PlusOutlined />} onClick={() => open()}>
            {i18nText('settings', 'organization.add_root')}
          </Button>
        ) : null}
      </div>
      {query.isError || remove.isError ? (
        <Alert
          type="error"
          message={i18nText('settings', 'organization.operation_error')}
          action={
            <Button
              onClick={() => {
                remove.reset();
                void query.refetch();
              }}
            >
              {i18nText('settings', 'auto.retry_permission_data')}
            </Button>
          }
        />
      ) : null}
      <Table<DepartmentTreeRow>
        rowKey="id"
        loading={query.isLoading}
        dataSource={tree}
        pagination={false}
        scroll={{ x: 760 }}
        expandable={{ defaultExpandAllRows: true }}
        locale={{ emptyText: i18nText('settings', 'organization.empty') }}
        columns={[
          {
            title: i18nText('settings', 'organization.name'),
            dataIndex: 'name',
            width: 260
          },
          {
            title: i18nText('settings', 'organization.count'),
            dataIndex: 'member_count',
            width: 100
          },
          {
            title: i18nText('settings', 'organization.department_roles'),
            dataIndex: 'role_codes',
            render: (codes: string[]) => codes.join(', ') || '—'
          },
          {
            title: i18nText('settings', 'auto.operation'),
            key: 'actions',
            width: 280,
            render: (_, department) => (
              <Space>
                {access.can_create ? (
                  <Button
                    size="small"
                    onClick={() => open(undefined, department.id)}
                  >
                    {i18nText('settings', 'organization.add_child')}
                  </Button>
                ) : null}
                {access.can_update ? (
                  <Button size="small" onClick={() => open(department)}>
                    {i18nText('settings', 'auto.edit')}
                  </Button>
                ) : null}
                {access.can_delete ? (
                  <Popconfirm
                    title={i18nText('settings', 'organization.delete_confirm')}
                    onConfirm={() => remove.mutate(department.id)}
                  >
                    <Button size="small" danger loading={remove.isPending}>
                      {i18nText('settings', 'auto.delete')}
                    </Button>
                  </Popconfirm>
                ) : null}
              </Space>
            )
          }
        ]}
      />
      {draft ? (
        <DepartmentEditorDrawer
          draft={draft}
          departments={data}
          canAssignRoles={access.can_assign_roles}
          onClose={() => setDraft(undefined)}
        />
      ) : null}
    </section>
  );
}
