import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Form, Button } from 'antd';
import { describe, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../../app/AppProviders';
import { MemberDepartmentFields } from '../MemberDepartmentFields';
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
  invalid = false
}: {
  submit: (value: unknown) => void;
  invalid?: boolean;
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
      <MemberDepartmentFields form={form} departments={departments} />
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
