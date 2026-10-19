import { act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { Form, Button } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../../app/AppProviders';
import { MemberDepartmentFields } from '../MemberDepartmentFields';
const fetchDepartments = vi.hoisted(() => vi.fn());
vi.mock('../../../api/identity-access/departments', () => ({
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
  beforeEach(() => {
    fetchDepartments.mockImplementation(async ({ ids }: { ids?: string }) => ({
      items: departments
        .filter((item) => !ids || ids.split(',').includes(item.id))
        .map((item) => ({ ...item, is_match: true, has_children: false })),
      has_more: false,
      next_cursor: null
    }));
  });
  afterEach(async () => {
    await act(async () => {
      // Dispose only this fixture's render roots and their motion subscriptions.
      for (const view of mountedViews) view.unmount();
      mountedViews.clear();
    });
  });
  test('retains and resolves existing selections absent from the first page', async () => {
    const submit = vi.fn();
    await renderSettled(
      <AppProviders>
        <Harness submit={submit} partial />
      </AppProviders>
    );
    expect(await screen.findAllByText('Support')).not.toHaveLength(0);
    expect(fetchDepartments).toHaveBeenCalledWith({ ids: 'a,b', limit: 50 });
    await domEvents.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'a'
      })
    );
  });
  test('edits the primary department while retaining both assignments', async () => {
    const submit = vi.fn();
    await renderSettled(
      <AppProviders>
        <Harness submit={submit} />
      </AppProviders>
    );
    await domEvents.mouseDown(screen.getByRole('combobox', { name: '主部门' }));
    const support = await screen.findByText((_, element) =>
      Boolean(
        element?.matches('.ant-select-item-option-content') &&
        element.textContent === 'Support'
      )
    );
    const popup = support.closest<HTMLElement>('.ant-select-dropdown');
    if (!popup) throw new Error('Primary department selector popup is missing');
    await domEvents.click(support);
    await waitFor(() => expect(popup).not.toBeVisible());
    await domEvents.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'b'
      })
    );
  });
  test('saves exactly one primary department from the selected set', async () => {
    const submit = vi.fn();
    await renderSettled(
      <AppProviders>
        <Harness submit={submit} />
      </AppProviders>
    );
    await domEvents.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: ['a', 'b'],
        primary_department_id: 'a'
      })
    );
  });
  test('rejects a primary outside the selected set and permits an explicit empty assignment', async () => {
    const submit = vi.fn();
    await renderSettled(
      <AppProviders>
        <Harness submit={submit} invalid />
      </AppProviders>
    );
    await domEvents.click(screen.getByText('Submit'));
    expect(
      await screen.findByText('请选择所属部门中的一个主部门')
    ).toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
    await domEvents.click(screen.getByText('Clear'));
    await domEvents.click(screen.getByText('Submit'));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        department_ids: [],
        primary_department_id: null
      })
    );
  });
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => { result = event(); });
  return result;
}

// Explicit render owners for this fixture; other test roots remain RTL-owned.
const mountedViews = new Set<ReturnType<typeof render>>();

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(...args);
    mountedViews.add(view);
  });
  return view;
}

const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) => dispatch(() => fireEvent.click(...args)),
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) => dispatch(() => fireEvent.mouseDown(...args))
};
