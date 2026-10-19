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
    submitting
  }: {
    me: ConsoleMe;
    onSubmit: (input: unknown) => Promise<void>;
    submitting: boolean;
  }) => (
    <div>
      <span>{me.name}</span>
      <button
        disabled={submitting}
        onClick={() => onSubmit({ name: 'Saved profile' })}
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
  permissions: []
};

afterEach(() => resetAuthStore());

describe('profile cache coherence', () => {
  it('updates the current profile cache and preserves the saved profile on remount', async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    client.setQueryData(['me', 'profile'], original);
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf',
      actor: {
        id: 'user-1',
        account: 'member',
        effective_display_role: 'member',
        current_workspace_id: 'workspace'
      },
      me: original
    });
    const saved = { ...original, name: 'Saved profile' };
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
});
