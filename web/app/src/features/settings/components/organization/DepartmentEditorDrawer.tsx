import { useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Drawer, Form, Input, Select } from 'antd';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  createSettingsDepartment,
  updateSettingsDepartment,
  fetchSettingsMemberRoleOptions,
  settingsDepartmentsQueryKey,
  settingsMemberRoleOptionsQueryKey,
  type DepartmentInput,
  type SettingsDepartment
} from '../../api/departments';
import { settingsMembersQueryKey } from '../../api/members';

export interface DepartmentDraft {
  department?: SettingsDepartment;
  parent_id: string | null;
}

export function DepartmentEditorDrawer({
  draft,
  departments,
  canAssignRoles,
  onClose
}: {
  draft: DepartmentDraft;
  departments: SettingsDepartment[];
  canAssignRoles: boolean;
  onClose: () => void;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const client = useQueryClient();
  const [form] = Form.useForm<DepartmentInput>();
  const rolesQuery = useQuery({
    queryKey: settingsMemberRoleOptionsQueryKey,
    queryFn: fetchSettingsMemberRoleOptions,
    enabled: canAssignRoles
  });
  const save = useMutation({
    mutationFn: (values: DepartmentInput) => {
      if (!csrfToken) throw new Error('missing csrf token');
      const input = {
        name: values.name.trim(),
        parent_id: values.parent_id ?? null,
        role_codes: canAssignRoles
          ? (values.role_codes ?? [])
          : (draft.department?.role_codes ?? [])
      };
      return draft.department
        ? updateSettingsDepartment(draft.department.id, input, csrfToken)
        : createSettingsDepartment(input, csrfToken);
    },
    onSuccess: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: settingsDepartmentsQueryKey }),
        client.invalidateQueries({ queryKey: settingsMembersQueryKey })
      ]);
      onClose();
    }
  });
  const excluded = useMemo(() => {
    const ids = new Set<string>();
    if (draft.department) {
      ids.add(draft.department.id);
      let changed = true;
      while (changed) {
        changed = false;
        for (const department of departments) {
          if (
            department.parent_id &&
            ids.has(department.parent_id) &&
            !ids.has(department.id)
          ) {
            ids.add(department.id);
            changed = true;
          }
        }
      }
    }
    return ids;
  }, [departments, draft.department]);
  return (
    <Drawer
      title={
        draft?.department
          ? i18nText('settings', 'organization.edit')
          : i18nText('settings', 'organization.create')
      }
      open
      onClose={onClose}
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
        initialValues={{
          name: draft.department?.name ?? '',
          parent_id: draft.department?.parent_id ?? draft.parent_id,
          role_codes: draft.department?.role_codes ?? []
        }}
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
            options={departments
              .filter((department) => !excluded.has(department.id))
              .map((department) => ({
                label: department.name,
                value: department.id
              }))}
          />
        </Form.Item>
        {canAssignRoles ? (
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
  );
}
