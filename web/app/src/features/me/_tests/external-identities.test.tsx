import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, test, vi } from 'vitest';
import { ExternalIdentitiesPanel } from '../components/ExternalIdentitiesPanel';

const mocks = vi.hoisted(() => ({ entries: vi.fn(), authorize: vi.fn() }));
vi.mock('../../auth/api/session', () => ({ fetchLoginEntries: mocks.entries }));
vi.mock('../api/me', () => ({ authorizeMyExternalIdentity: mocks.authorize }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { provider?: string }) =>
      values?.provider ? `Link ${values.provider}` : key
  })
}));
afterEach(() => vi.resetAllMocks());
function renderPanel(csrf: string | null) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  return render(
    <QueryClientProvider client={client}>
      <ExternalIdentitiesPanel csrfToken={csrf} />
    </QueryClientProvider>
  );
}
test('only external entries can be bound and the current CSRF is required', async () => {
  mocks.entries.mockResolvedValue({
    login_entries: [
      { id: 'password', auth_type: 'password-local', title: 'Password' },
      { id: 'github', auth_type: '1flowbase/github-login', title: 'GitHub' }
    ]
  });
  mocks.authorize.mockRejectedValue(new Error('safe authorization failure'));
  renderPanel('fixture-csrf');
  fireEvent.click(await screen.findByRole('button', { name: 'Link GitHub' }));
  await waitFor(() =>
    expect(mocks.authorize).toHaveBeenCalledWith('github', 'fixture-csrf')
  );
  expect(
    screen.queryByRole('button', { name: 'Link Password' })
  ).not.toBeInTheDocument();
  expect(
    await screen.findByText('external_identities.failed')
  ).toBeInTheDocument();
});
test('an absent session cannot authorize an identity binding', async () => {
  mocks.entries.mockResolvedValue({
    login_entries: [
      { id: 'github', auth_type: '1flowbase/github-login', title: 'GitHub' }
    ]
  });
  renderPanel(null);
  expect(
    await screen.findByRole('button', { name: 'Link GitHub' })
  ).toBeDisabled();
  expect(mocks.authorize).not.toHaveBeenCalled();
});
