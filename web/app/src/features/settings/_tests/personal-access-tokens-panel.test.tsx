import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const personalAccessTokensApi = vi.hoisted(() => ({
  settingsPersonalAccessTokensQueryKey: ['settings', 'personal-access-tokens'],
  settingsPersonalAccessTokenRoleOptionsQueryKey: [
    'settings',
    'personal-access-tokens',
    'role-options'
  ],
  fetchSettingsPersonalAccessTokens: vi.fn(),
  fetchSettingsPersonalAccessTokenRoleOptions: vi.fn(),
  createSettingsPersonalAccessToken: vi.fn(),
  revokeSettingsPersonalAccessToken: vi.fn()
}));

vi.mock('../api/personal-access-tokens', () => personalAccessTokensApi);

import { AppProviders } from '../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { PersonalAccessTokensPanel } from '../components/PersonalAccessTokensPanel';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: []
    }
  });
}

function renderPanel() {
  return render(
    <AppProviders>
      <PersonalAccessTokensPanel />
    </AppProviders>
  );
}

describe('PersonalAccessTokensPanel', () => {
  beforeEach(() => {
    resetAuthStore();
    authenticate();
    personalAccessTokensApi.fetchSettingsPersonalAccessTokens.mockResolvedValue(
      [
        {
          id: 'key-1',
          name: 'Existing automation',
          token: null,
          token_prefix: 'pat_abc',
          key_kind: 'user_api_key',
          role_code: 'root',
          creator_user_id: 'user-1',
          tenant_id: 'tenant-1',
          scope_kind: 'workspace',
          scope_id: 'workspace-1',
          enabled: true,
          revoked: false,
          expires_at: '2027-06-22T00:00:00Z',
          last_used_at: null,
          created_at: '2026-06-22T00:00:00Z',
          updated_at: '2026-06-22T00:00:00Z'
        },
        {
          id: 'key-2',
          name: 'Deleted automation',
          token: null,
          token_prefix: 'pat_deleted',
          key_kind: 'user_api_key',
          role_code: 'root',
          creator_user_id: 'user-1',
          tenant_id: 'tenant-1',
          scope_kind: 'workspace',
          scope_id: 'workspace-1',
          enabled: false,
          revoked: true,
          expires_at: '2027-06-21T00:00:00Z',
          last_used_at: null,
          created_at: '2026-06-21T00:00:00Z',
          updated_at: '2026-06-21T00:00:00Z'
        }
      ]
    );
    personalAccessTokensApi.createSettingsPersonalAccessToken.mockResolvedValue(
      {
        id: 'key-2',
        name: 'CI diagnostics',
        token: 'pat_new_secret',
        token_prefix: 'pat_new',
        key_kind: 'user_api_key',
        role_code: 'root',
        creator_user_id: 'user-1',
        tenant_id: 'tenant-1',
        scope_kind: 'workspace',
        scope_id: 'workspace-1',
        enabled: true,
        revoked: false,
        expires_at: null,
        last_used_at: null,
        created_at: '2026-06-22T00:00:00Z',
        updated_at: '2026-06-22T00:00:00Z'
      }
    );
    personalAccessTokensApi.fetchSettingsPersonalAccessTokenRoleOptions.mockResolvedValue(
      [{ code: 'root', name: 'Root', scope_kind: 'system' }]
    );
    personalAccessTokensApi.revokeSettingsPersonalAccessToken.mockResolvedValue(
      undefined
    );
  });

  test('lists token metadata without leaking full token values', async () => {
    renderPanel();

    expect(await screen.findByText('Existing automation')).toBeInTheDocument();
    expect(screen.getByText('pat_abc')).toBeInTheDocument();
    expect(screen.queryByText('pat_new_secret')).not.toBeInTheDocument();
  });

  test('shows active API keys by default and filters deleted keys on query', async () => {
    renderPanel();

    expect(await screen.findByText('Existing automation')).toBeInTheDocument();
    expect(screen.queryByText('Deleted automation')).not.toBeInTheDocument();

    fireEvent.mouseDown(screen.getByLabelText('状态'));
    fireEvent.click(await screen.findByText('已删除'));
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));

    expect(await screen.findByText('Deleted automation')).toBeInTheDocument();
    expect(screen.queryByText('Existing automation')).not.toBeInTheDocument();
  });

  test('creates a token, shows the secret once, and refreshes the list', async () => {
    renderPanel();
    await screen.findByText('Existing automation');
    const listCallsBeforeCreate =
      personalAccessTokensApi.fetchSettingsPersonalAccessTokens.mock.calls
        .length;

    fireEvent.click(
      await screen.findByRole('button', { name: /添加/ })
    );
    fireEvent.change(screen.getByLabelText('名称'), {
      target: { value: 'CI diagnostics' }
    });
    fireEvent.mouseDown(screen.getByLabelText('有效期'));
    fireEvent.click(await screen.findByText('永不过期'));
    fireEvent.click(screen.getByRole('button', { name: /创\s*建/ }));

    await waitFor(() => {
      expect(
        personalAccessTokensApi.createSettingsPersonalAccessToken
      ).toHaveBeenCalledWith(
        {
          name: 'CI diagnostics',
          role_code: 'root',
          expiration_policy: 'never'
        },
        'csrf-123'
      );
    });

    expect(await screen.findByText('pat_new_secret')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /复制/ })).toBeInTheDocument();
    await waitFor(() => {
      expect(
        personalAccessTokensApi.fetchSettingsPersonalAccessTokens.mock.calls
          .length
      ).toBeGreaterThan(listCallsBeforeCreate);
    });
  }, 10_000);

  test('deletes active tokens through the revoke action', async () => {
    renderPanel();

    const row = await screen.findByRole('row', {
      name: /Existing automation.*pat_abc/u
    });
    fireEvent.click(within(row).getByRole('button', { name: /删除/ }));
    fireEvent.click(await screen.findByRole('button', { name: '确认删除' }));

    await waitFor(() => {
      expect(
        personalAccessTokensApi.revokeSettingsPersonalAccessToken
      ).toHaveBeenCalledWith('key-1', 'csrf-123');
    });
  });
});
