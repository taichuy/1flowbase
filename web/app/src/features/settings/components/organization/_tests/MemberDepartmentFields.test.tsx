import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Form, Button } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../../app/AppProviders';
import { MemberDepartmentFields } from '../MemberDepartmentFields';
const fetchDepartments = vi.hoisted(() => vi.fn());
vi.mock('../../../api/departments', () => ({
  fetchSettingsDepartments: fetchDepartments,
  settingsDepartmentsQueryKey: ['settings', 'departments']
}));
const departments = [
  {
    id: 'a',
    name: 'Engineering',
    parent_id: null,
    role_codes: [],
    member_count: 7
  },
  { id: 'b', name: 'Support', parent_id: null, role_codes: [], member_count: 4 }
];
function Harness({
  submit,
  invalid = false,
  partial = false
}: {
  submit: (value: unknown) => void;
  invalid?: boolean;
  partial?: boolean;
}) {
  const [form] = Form.useForm();
  return (
    <Form
      form={form}
      initialValues={{
        department_ids: ['a', 'b'],
        primary_department_id: invalid ? 'outside' : 'a'
      }}
      onFinish={submit}
    >
      <MemberDepartmentFields
        form={form}
        departments={partial ? [] : departments}
      />
      <Button onClick={() => form.submit()}>Submit</Button>
      <Button
        onClick={() => {
          form.setFieldsValue({
            department_ids: [],
            primary_department_id: null
          });
        }}
      >
        Clear
      </Button>
    </Form>
  );
}
describe('member department invariants', () => {
  beforeEach(() =>
    fetchDepartments.mockImplementation(async ({ ids }: { ids?: string }) => ({
      items: departments
        .filter((item) => !ids || ids.split(',').includes(item.id))
        .map((item) => ({ ...item, is_match: true, has_children: false })),
      has_more: false,
      next_cursor: null
    }))
  );
  test('retains and resolves existing selections absent from the first page', async () => {
    const submit = vi.fn();
    render(
      <AppProviders>
        <Harness submit={submit} partial />
      </AppProviders>
    );
    expect(await screen.findAllByText('Support')).not.toHaveLength(0);
    expect(fetchDepartments).toHaveBeenCalledWith({ ids: 'a,b', limit: 50 });
    fireEvent.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'a'
      })
    );
  });
  test('edits the primary department while retaining both assignments', async () => {
    const submit = vi.fn();
    render(
      <AppProviders>
        <Harness submit={submit} />
      </AppProviders>
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '主部门' }));
    fireEvent.click(
      await screen.findByText((_, element) =>
        Boolean(
          element?.matches('.ant-select-item-option-content') &&
          element.textContent === 'Support'
        )
      )
    );
    fireEvent.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'b'
      })
    );
  });
  test('saves exactly one primary department from the selected set', async () => {
    const submit = vi.fn();
    render(
      <AppProviders>
        <Harness submit={submit} />
      </AppProviders>
    );
    fireEvent.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'a'
      })
    );
  });
  test('rejects a primary outside the selected set and permits an explicit empty assignment', async () => {
    const submit = vi.fn();
    render(
      <AppProviders>
        <Harness submit={submit} invalid />
      </AppProviders>
    );
    fireEvent.click(screen.getByText('Submit'));
    expect(
      await screen.findByText('请选择所属部门中的一个主部门')
    ).toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('Clear'));
    fireEvent.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: [],
        primary_department_id: null
      })
    );
  });
});
