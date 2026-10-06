import { Form, Select, Typography } from 'antd';
import type { FormInstance } from 'antd';
import type { SettingsDepartment } from '../../api/departments';
import { i18nText } from '../../../../shared/i18n/text';
export function MemberDepartmentFields({
  form,
  departments,
  disabled = false
}: {
  form: FormInstance;
  departments: SettingsDepartment[];
  disabled?: boolean;
}) {
  const department_ids: string[] = Form.useWatch('department_ids', form) ?? [];
  return (
    <>
      <Form.Item
        name="department_ids"
        label={i18nText('settings', 'organization.member_departments')}
      >
        <Select
          mode="multiple"
          disabled={disabled}
          options={departments.map((department) => ({
            value: department.id,
            label: department.name
          }))}
          onChange={(ids: string[]) => {
            const primary: string | null = form.getFieldValue(
              'primary_department_id'
            );
            if (!primary || !ids.includes(primary))
              form.setFieldValue('primary_department_id', ids[0] ?? null);
          }}
        />
      </Form.Item>
      <Form.Item
        name="primary_department_id"
        label={i18nText('settings', 'organization.primary_department')}
        dependencies={['department_ids']}
        rules={[
          {
            validator: (_, value) => {
              const ids: string[] = form.getFieldValue('department_ids') ?? [];
              return (ids.length === 0 && !value) || ids.includes(value)
                ? Promise.resolve()
                : Promise.reject(
                    new Error(
                      i18nText('settings', 'organization.primary_required')
                    )
                  );
            }
          }
        ]}
      >
        <Select
          disabled={disabled || !department_ids.length}
          options={departments
            .filter((department) => department_ids.includes(department.id))
            .map((department) => ({
              value: department.id,
              label: department.name
            }))}
        />
      </Form.Item>
      <Typography.Paragraph type="secondary">
        {i18nText('settings', 'organization.inherited_roles_hint')}
      </Typography.Paragraph>
      {departments
        .filter(
          (department) =>
            department_ids.includes(department.id) &&
            department.role_codes.length > 0
        )
        .map((department) => (
          <Typography.Paragraph type="secondary" key={department.id}>
            {department.name}: {department.role_codes.join(', ')}
          </Typography.Paragraph>
        ))}
    </>
  );
}
