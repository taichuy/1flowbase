import { cleanup,
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  fetchSettingsDepartments: vi.fn(),
  createSettingsDepartment: vi.fn(),
  updateSettingsDepartment: vi.fn(),
  fetchSettingsMemberRoleOptions: vi.fn(),
  settingsDepartmentsQueryKey: ['settings', 'departments'],
  settingsMemberRoleOptionsQueryKey: ['settings', 'members', 'role-options']
}));
vi.mock('../../../api/departments', () => api);

import { AppProviders } from '../../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import {
  DepartmentEditorDrawer,
  type DepartmentDraft
} from '../DepartmentEditorDrawer';

const engineering = {
  id: 'engineering',
  name: 'Engineering',
  parent_id: null,
  role_codes: ['operator'],
  member_count: 3
};
const development = {
  id: 'development',
  name: 'Development',
  parent_id: 'engineering',
  role_codes: ['member'],
  member_count: 1
};

async function renderEditor(
  draft: DepartmentDraft,
  departments = [engineering, development]
) {
  await renderSettled(
    <AppProviders>
      <DepartmentEditorDrawer
        draft={draft}
        departments={departments}
        canAssignRoles={false}
        onClose={vi.fn()}
      />
    </AppProviders>
  );
}

function parentSelect() {
  const select = screen
    .getByRole('combobox', { name: '上级部门' })
    .closest<HTMLElement>('.ant-select');
  if (!select) throw new Error('Parent selector is missing');
  return select;
}

async function chooseOrganization() {
  await domEvents.mouseDown(screen.getByRole('combobox', { name: '上级部门' }));
  const [root] = await screen.findAllByText((_, element) =>
    Boolean(
      element?.matches('.ant-select-item-option-content') &&
      element.textContent === '组织'
    )
  );
  await domEvents.click(root);
}

async function save() {
  await domEvents.click(screen.getByRole('button', { name: /保\s*存/ }));
}

describe('organization parent selector', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.clearAllMocks();
    resetAuthStore();
    useAuthStore.setState({ csrfToken: 'csrf' });
    api.fetchSettingsDepartments.mockImplementation(
      async ({ ids }: { ids?: string }) => ({
        items: [engineering, development]
          .filter((item) => !ids || ids.split(',').includes(item.id))
          .map((item) => ({
            ...item,
            has_children: item.id === 'engineering',
            is_match: true
          })),
        has_more: false,
        next_cursor: null
      })
    );
    api.createSettingsDepartment.mockResolvedValue({ id: 'created' });
    api.updateSettingsDepartment.mockResolvedValue(development);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  test('an empty organization tree displays and offers the organization root and submits null', async () => {
    await renderEditor({ parent_id: null }, []);
    expect(within(parentSelect()).getByText('组织')).toBeInTheDocument();
    await chooseOrganization();
    await domEvents.change(screen.getByLabelText('部门名称'), {
      target: { value: 'First department' }
    });
    await save();
    await waitFor(() =>
      expect(api.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'First department', parent_id: null, role_codes: [] },
        'csrf'
      )
    );
  });

  test('creation keeps a concrete default parent unchanged', async () => {
    await renderEditor({ parent_id: 'engineering' });
    expect(within(parentSelect()).getByText('Engineering')).toBeInTheDocument();
    await domEvents.change(screen.getByLabelText('部门名称'), {
      target: { value: 'Child' }
    });
    await save();
    await waitFor(() =>
      expect(api.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'Child', parent_id: 'engineering', role_codes: [] },
        'csrf'
      )
    );
  });

  test('resolves a current parent outside the loaded page', async () => {
    await renderEditor({ department: development, parent_id: null }, []);
    await waitFor(() =>
      expect(
        within(parentSelect()).getByText('Engineering')
      ).toBeInTheDocument()
    );
    expect(api.fetchSettingsDepartments).toHaveBeenCalledWith({
      ids: 'engineering',
      limit: 50
    });
    await save();
    await waitFor(() =>
      expect(api.updateSettingsDepartment).toHaveBeenCalledWith(
        'development',
        {
          name: 'Development',
          parent_id: 'engineering',
          role_codes: ['member']
        },
        'csrf'
      )
    );
  });

  test('editing preserves the existing concrete parent and department roles', async () => {
    await renderEditor({ department: development, parent_id: null });
    expect(within(parentSelect()).getByText('Engineering')).toBeInTheDocument();
    await save();
    await waitFor(() =>
      expect(api.updateSettingsDepartment).toHaveBeenCalledWith(
        'development',
        {
          name: 'Development',
          parent_id: 'engineering',
          role_codes: ['member']
        },
        'csrf'
      )
    );
  });

  test.each(['choose root', 'clear'] as const)(
    '%s moves an edited department to the virtual organization root using null',
    async (action) => {
      await renderEditor({ department: development, parent_id: null });
      expect(
        within(parentSelect()).getByText('Engineering')
      ).toBeInTheDocument();
      if (action === 'choose root') {
        await chooseOrganization();
      } else {
        const clear =
          parentSelect().querySelector<HTMLElement>('.ant-select-clear');
        if (!clear) throw new Error('Parent clear action is missing');
        await domEvents.click(clear);
      }
      expect(within(parentSelect()).getByText('组织')).toBeInTheDocument();
      await save();
      await waitFor(() =>
        expect(api.updateSettingsDepartment).toHaveBeenCalledWith(
          'development',
          { name: 'Development', parent_id: null, role_codes: ['member'] },
          'csrf'
        )
      );
    }
  );
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => { result = event(); });
  return result;
}

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => { view = render(...args); });
  return view;
}

const domEvents = {
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) => dispatch(() => fireEvent.mouseDown(...args)),
  click: (...args: Parameters<typeof fireEvent.click>) => dispatch(() => fireEvent.click(...args)),
  change: (...args: Parameters<typeof fireEvent.change>) => dispatch(() => fireEvent.change(...args))
};
