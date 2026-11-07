import { beforeEach, describe, expect, test, vi } from 'vitest';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import {
  ApiClientError,
  fetchRecordPermissions,
  replaceRecordPermissions
} from '@1flowbase/api-client';
import { RecordPermissionEditor } from '../RecordPermissionEditor';
import { policy, role } from './fixtures';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  fetchRecordPermissions: vi.fn(),
  replaceRecordPermissions: vi.fn()
}));

const key = { plugin_code: 'ssh', resource_code: 'hosts', record_id: 'host-1' };
function editor(locale = 'en_US') {
  const callbacks = { onCancel: vi.fn(), onSaved: vi.fn() };
  render(
    <RecordPermissionEditor
      pluginCode="ssh"
      resourceCode="hosts"
      recordId="host-1"
      open
      locale={locale}
      {...callbacks}
    />
  );
  return callbacks;
}
async function chooseMode(text: string, name = 'Permission mode') {
  fireEvent.mouseDown(screen.getByRole('combobox', { name }));
  fireEvent.click(
    await screen.findByText(text, {
      selector: '.ant-select-item-option-content'
    })
  );
}

beforeEach(async () => {
  vi.clearAllMocks();
  resetAuthStore();
  useAuthStore.setState({ csrfToken: 'session-csrf' });
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('en_US');
  vi.mocked(fetchRecordPermissions).mockResolvedValue(policy());
  vi.mocked(replaceRecordPermissions).mockResolvedValue({
    mode: 'inherit',
    grants: [],
    revision: 1
  });
});

describe('RecordPermissionEditor', () => {
  test('keeps inherited permission mode explicit and sends no role policy on cancel', async () => {
    const callbacks = editor();
    expect(
      await screen.findByText(
        'This record uses the roles’ existing API permissions.'
      )
    ).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(callbacks.onCancel).toHaveBeenCalledOnce();
    expect(replaceRecordPermissions).not.toHaveBeenCalled();
  });

  test('saving inherit clears grants and preserves optimistic revision', async () => {
    vi.mocked(fetchRecordPermissions).mockResolvedValue(
      policy({ revision: 9 })
    );
    const callbacks = editor();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(replaceRecordPermissions).toHaveBeenCalledWith(
        key,
        { mode: 'inherit', grants: [], expected_revision: 9 },
        'session-csrf'
      )
    );
    expect(callbacks.onSaved).toHaveBeenCalledOnce();
  });

  test('restricting with no selected role saves restricted empty grants, never inherit', async () => {
    editor();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    await chooseMode('Restrict this record');
    expect(screen.getByText(/With no roles selected, only root/)).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(replaceRecordPermissions).toHaveBeenCalledWith(
        key,
        { mode: 'restricted', grants: [], expected_revision: 0 },
        'session-csrf'
      )
    );
  });

  test('edits each of the four actions on the backend role UUID', async () => {
    vi.mocked(fetchRecordPermissions).mockResolvedValue(
      policy({
        mode: 'restricted',
        grants: [{ role_id: role.role_id, actions: [] }]
      })
    );
    editor();
    for (const label of ['View', 'Edit', 'Open', 'Delete']) {
      fireEvent.click(
        await screen.findByRole('checkbox', { name: `Operator ${label}` })
      );
    }
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(replaceRecordPermissions).toHaveBeenCalledWith(
        key,
        {
          mode: 'restricted',
          grants: [
            {
              role_id: role.role_id,
              actions: ['view', 'update', 'open', 'delete']
            }
          ],
          expected_revision: 0
        },
        'session-csrf'
      )
    );
  });

  test('chooses a current-workspace role without inventing a display identity', async () => {
    vi.mocked(fetchRecordPermissions).mockResolvedValue(
      policy({ mode: 'restricted' })
    );
    editor();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Roles' }));
    fireEvent.click(
      await screen.findByText('Operator', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Operator View' })
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(replaceRecordPermissions).toHaveBeenCalledWith(
        key,
        {
          mode: 'restricted',
          grants: [{ role_id: role.role_id, actions: ['view'] }],
          expected_revision: 0
        },
        'session-csrf'
      )
    );
  });

  test('conflicts keep the draft, block stale saves and reload the current revision', async () => {
    vi.mocked(fetchRecordPermissions).mockResolvedValue(
      policy({
        mode: 'restricted',
        grants: [{ role_id: role.role_id, actions: ['view'] }],
        revision: 3
      })
    );
    vi.mocked(replaceRecordPermissions).mockRejectedValueOnce(
      new ApiClientError({ status: 409, message: 'conflict' })
    );
    const callbacks = editor();
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Operator Open' })
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(
      await screen.findByText('Permissions have changed. Reload before saving.')
    ).toBeVisible();
    expect(
      screen.getByRole('checkbox', { name: 'Operator Open' })
    ).toBeChecked();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(callbacks.onSaved).not.toHaveBeenCalled();
    vi.mocked(fetchRecordPermissions).mockResolvedValue(
      policy({ revision: 4 })
    );
    fireEvent.click(screen.getByRole('button', { name: 'Reload' }));
    await waitFor(() =>
      expect(fetchRecordPermissions).toHaveBeenCalledTimes(2)
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    );
  });

  test('switching workspace discards a late response from the previous caller scope', async () => {
    let finishPrevious!: (value: ReturnType<typeof policy>) => void;
    vi.mocked(fetchRecordPermissions)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finishPrevious = resolve;
          })
      )
      .mockResolvedValueOnce(policy({ revision: 8 }));
    editor();
    await waitFor(() =>
      expect(fetchRecordPermissions).toHaveBeenCalledTimes(1)
    );
    act(() =>
      useAuthStore.setState({
        actor: {
          id: 'user-1',
          account: 'operator',
          current_workspace_id: 'workspace-b',
          effective_display_role: 'operator'
        }
      })
    );
    await waitFor(() =>
      expect(fetchRecordPermissions).toHaveBeenCalledTimes(2)
    );
    await act(async () =>
      finishPrevious(
        policy({
          mode: 'restricted',
          revision: 99,
          grants: [{ role_id: role.role_id, actions: ['open'] }]
        })
      )
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(replaceRecordPermissions).toHaveBeenCalledWith(
        key,
        { mode: 'inherit', grants: [], expected_revision: 8 },
        'session-csrf'
      )
    );
  });

  test.each(['zh_Hans', 'en_US'])(
    'renders localized four-action labels in %s',
    async (locale) => {
      vi.mocked(fetchRecordPermissions).mockResolvedValue(
        policy({
          mode: 'restricted',
          grants: [{ role_id: role.role_id, actions: ['view'] }]
        })
      );
      editor(locale);
      const labels =
        locale === 'zh_Hans'
          ? ['查看', '编辑', '打开', '删除']
          : ['View', 'Edit', 'Open', 'Delete'];
      for (const label of labels)
        expect(
          await screen.findByRole('checkbox', { name: `Operator ${label}` })
        ).toBeVisible();
      expect(
        within(screen.getByRole('dialog')).getByText(
          locale === 'zh_Hans' ? '权限' : 'Permissions'
        )
      ).toBeVisible();
    }
  );
});
