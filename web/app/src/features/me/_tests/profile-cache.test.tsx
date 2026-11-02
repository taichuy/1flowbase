import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { PropsWithChildren } from 'react';
import type { ConsoleMe } from '@1flowbase/api-client';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { MePage } from '../pages/MePage';

const { updateMyProfile } = vi.hoisted(() => ({ updateMyProfile: vi.fn() }));
vi.mock('../api/me', () => ({
  updateMyProfile,
  fetchMyProfile: vi.fn(),
  changeMyPassword: vi.fn()
}));
vi.mock('@tanstack/react-router', () => ({
  Navigate: () => null,
  useNavigate: () => vi.fn()
}));
vi.mock('../../../shared/ui/section-page-layout/SectionPageLayout', () => ({
  SectionPageLayout: ({ children }: PropsWithChildren) => children
}));
vi.mock('../components/ProfileForm', () => ({
  ProfileForm: ({
    me,
    onSubmit,
    submitting,
    errorMessage
  }: {
    me: ConsoleMe;
    onSubmit: (input: unknown) => Promise<void>;
    submitting: boolean;
    errorMessage: string | null;
  }) => (
    <div>
      <span>{me.name}</span>
      <span>{errorMessage}</span>
      <button
        disabled={submitting}
        onClick={() => void onSubmit({ name: 'Saved profile' }).catch(() => {})}
      >
        Save profile
      </button>
    </div>
  )
}));

const original: ConsoleMe = {
  id: 'user-1',
  account: 'member',
  email: 'member@example.com',
  phone: null,
  nickname: 'Member',
  name: 'Original profile',
  avatar_url: null,
  introduction: '',
  effective_display_role: 'member',
  permissions: ['console.identity.me.patch']
};

afterEach(() => { resetAuthStore(); updateMyProfile.mockReset(); });

describe('profile cache coherence', () => {
  it.each(['root', 'single-operation'])('updates the profile cache for %s and preserves it on remount', async (grant) => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const grantedProfile = { ...original, permissions: grant === 'root' ? [] : original.permissions };
    client.setQueryData(['me', 'profile'], grantedProfile);
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf',
      actor: {
        id: 'user-1',
        account: 'member',
        effective_display_role: grant === 'root' ? 'root' : 'member',
        current_workspace_id: 'workspace'
      },
      me: grantedProfile
    });
    const saved = { ...grantedProfile, name: 'Saved profile' };
    updateMyProfile.mockResolvedValue(saved);
    const wrapper = ({ children }: PropsWithChildren) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const view = render(<MePage requestedSectionKey="profile" />, { wrapper });
    fireEvent.click(screen.getByRole('button', { name: 'Save profile' }));
    await waitFor(() =>
      expect(client.getQueryData(['me', 'profile'])).toEqual(saved)
    );
    expect(useAuthStore.getState().me).toEqual(saved);
    await waitFor(() => {
      expect(screen.getByText('Saved profile')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Save profile' })).toBeEnabled();
    });
    view.unmount();
    const { unmount: unmountRemounted } = render(<MePage requestedSectionKey="profile" />, {
      wrapper
    });
    expect(screen.getByText('Saved profile')).toBeInTheDocument();
    expect(useAuthStore.getState().me).toEqual(saved);
    // Unsubscribe both query and auth observers before resetting their owners.
    unmountRemounted();
    client.clear();
  });
  it.each([{ permissions: [] }, { permissions: ['console.identity.me.meta.patch'] }, { permissions: ['assistant.settings.update'] }])(
    'rejects a direct profile mutation with unrelated grants %j',
    async ({ permissions }) => {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const me = { ...original, permissions };
      client.setQueryData(['me', 'profile'], me);
      useAuthStore.getState().setAuthenticated({
        csrfToken: 'csrf',
        actor: { id: me.id, account: me.account, effective_display_role: 'member', current_workspace_id: 'workspace' },
        me
      });
      const view = render(<QueryClientProvider client={client}><MePage requestedSectionKey="profile" /></QueryClientProvider>);
      // The form stub deliberately invokes the mutation despite UI access restrictions.
      fireEvent.click(screen.getByRole('button', { name: 'Save profile' }));
      expect(await screen.findByText('operation access denied')).toBeInTheDocument();
      expect(updateMyProfile).not.toHaveBeenCalled();
      expect(client.getQueryData(['me', 'profile'])).toEqual(me);
      expect(useAuthStore.getState().me).toEqual(me);
      view.unmount();
      client.clear();
    }
  );

});
