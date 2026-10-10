import {
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

function renderEditor(
  draft: DepartmentDraft,
  departments = [engineering, development]
) {
  render(
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
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '上级部门' }));
  const [root] = await screen.findAllByText((_, element) =>
    Boolean(
      element?.matches('.ant-select-item-option-content') &&
      element.textContent === '组织'
    )
  );
  fireEvent.click(root);
}

function save() {
  fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
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

  afterEach(async () => {
    try {
      // Complete Form's delayed state updates while the fixture DOM is alive.
      // Mounted-tree cleanup remains owned by the test runner.
      await act(async () => {
        await vi.runOnlyPendingTimersAsync();
      });
    } finally {
      vi.useRealTimers();
    }
  });

  test('an empty organization tree displays and offers the organization root and submits null', async () => {
    renderEditor({ parent_id: null }, []);
    expect(within(parentSelect()).getByText('组织')).toBeInTheDocument();
    await chooseOrganization();
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: 'First department' }
    });
    save();
    await waitFor(() =>
      expect(api.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'First department', parent_id: null, role_codes: [] },
        'csrf'
      )
    );
  });

  test('creation keeps a concrete default parent unchanged', async () => {
    renderEditor({ parent_id: 'engineering' });
    expect(within(parentSelect()).getByText('Engineering')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: 'Child' }
    });
    save();
    await waitFor(() =>
      expect(api.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'Child', parent_id: 'engineering', role_codes: [] },
        'csrf'
      )
    );
  });

  test('resolves a current parent outside the loaded page', async () => {
    renderEditor({ department: development, parent_id: null }, []);
    await waitFor(() =>
      expect(
        within(parentSelect()).getByText('Engineering')
      ).toBeInTheDocument()
    );
    expect(api.fetchSettingsDepartments).toHaveBeenCalledWith({
      ids: 'engineering',
      limit: 50
    });
    save();
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
    renderEditor({ department: development, parent_id: null });
    expect(within(parentSelect()).getByText('Engineering')).toBeInTheDocument();
    save();
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
      renderEditor({ department: development, parent_id: null });
      expect(
        within(parentSelect()).getByText('Engineering')
      ).toBeInTheDocument();
      if (action === 'choose root') {
        await chooseOrganization();
      } else {
        const clear =
          parentSelect().querySelector<HTMLElement>('.ant-select-clear');
        if (!clear) throw new Error('Parent clear action is missing');
        fireEvent.click(clear);
      }
      expect(within(parentSelect()).getByText('组织')).toBeInTheDocument();
      save();
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
