import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Drawer,
  Empty,
  Form,
  Input,
  Popconfirm,
  Select,
  Space,
  Table,
  Typography
} from 'antd';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  createSettingsDepartment,
  deleteSettingsDepartment,
  fetchSettingsDepartments,
  settingsDepartmentsQueryKey,
  updateSettingsDepartment,
  type DepartmentInput,
  type DepartmentAccess,
  type SettingsDepartment
} from '../../api/departments';
import {
  fetchSettingsMemberRoleOptions,
  settingsMemberRoleOptionsQueryKey
} from '../../api/departments';
export type { DepartmentAccess } from '../../api/departments';
import { settingsMembersQueryKey } from '../../api/members';
import { departmentTree, type DepartmentTreeRow } from './department-tree';
import './organization-management.css';
export function DepartmentManagementPanel({
  access
}: {
  access: DepartmentAccess;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const client = useQueryClient();
  const [form] = Form.useForm<DepartmentInput>();
  const [draft, setDraft] = useState<{
    department?: SettingsDepartment;
    parent_id: string | null;
  }>();
  const query = useQuery({
    queryKey: settingsDepartmentsQueryKey,
    queryFn: fetchSettingsDepartments,
    enabled: access.can_list
  });
  const rolesQuery = useQuery({
    queryKey: settingsMemberRoleOptionsQueryKey,
    queryFn: fetchSettingsMemberRoleOptions,
    enabled: access.can_list && access.can_assign_roles
  });
  const data = query.data ?? [];
  const tree = useMemo(() => departmentTree(data), [data]);
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: settingsDepartmentsQueryKey }),
      client.invalidateQueries({ queryKey: settingsMembersQueryKey })
    ]);
  };
  const save = useMutation({
    mutationFn: async (values: DepartmentInput) => {
      if (!csrfToken) throw new Error('missing csrf token');
      const input = {
        name: values.name.trim(),
        parent_id: values.parent_id ?? null,
        role_codes: access.can_assign_roles
          ? (values.role_codes ?? [])
          : (draft?.department?.role_codes ?? [])
      };
      return draft?.department
        ? updateSettingsDepartment(draft.department.id, input, csrfToken)
        : createSettingsDepartment(input, csrfToken);
    },
    onSuccess: async () => {
      await refresh();
      setDraft(undefined);
      form.resetFields();
    }
  });
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
    save.reset();
    setDraft({ department, parent_id });
    form.setFieldsValue({
      name: department?.name ?? '',
      parent_id: department?.parent_id ?? parent_id,
      role_codes: department?.role_codes ?? []
    });
  };
  const excluded = new Set<string>();
  if (draft?.department) {
    excluded.add(draft.department.id);
    let changed = true;
    while (changed) {
      changed = false;
      for (const department of data)
        if (
          department.parent_id &&
          excluded.has(department.parent_id) &&
          !excluded.has(department.id)
        ) {
          excluded.add(department.id);
          changed = true;
        }
    }
  }
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
      <Drawer
        title={
          draft?.department
            ? i18nText('settings', 'organization.edit')
            : i18nText('settings', 'organization.create')
        }
        open={Boolean(draft)}
        onClose={() => {
          setDraft(undefined);
          form.resetFields();
        }}
        width={480}
        styles={{ wrapper: { maxWidth: '100vw' } }}
        extra={
          <Button
            type="primary"
            loading={save.isPending}
            onClick={() => form.submit()}
          >
            {i18nText('settings', 'auto.save')}
          </Button>
        }
      >
        {save.isError ? (
          <Alert
            type="error"
            message={i18nText('settings', 'organization.operation_error')}
          />
        ) : null}
        <Form
          form={form}
          layout="vertical"
          onFinish={(values) => save.mutate(values)}
        >
          <Form.Item
            name="name"
            label={i18nText('settings', 'organization.name')}
            rules={[
              {
                required: true,
                whitespace: true,
                message: i18nText('settings', 'organization.name_required')
              }
            ]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="parent_id"
            label={i18nText('settings', 'organization.parent')}
          >
            <Select
              allowClear
              options={data
                .filter((department) => !excluded.has(department.id))
                .map((department) => ({
                  label: department.name,
                  value: department.id
                }))}
            />
          </Form.Item>
          {access.can_assign_roles ? (
            <Form.Item
              name="role_codes"
              label={i18nText('settings', 'organization.department_roles')}
              extra={i18nText('settings', 'organization.inherited_roles_hint')}
            >
              <Select
                mode="multiple"
                loading={rolesQuery.isLoading}
                disabled={rolesQuery.isError}
                options={(rolesQuery.data ?? [])
                  .filter((role) => role.code !== 'root')
                  .map((role) => ({ label: role.name, value: role.code }))}
              />
            </Form.Item>
          ) : null}
        </Form>
      </Drawer>
    </section>
  );
}
