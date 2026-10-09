import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import type { PropsWithChildren } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { fetchFrontstageRuntimeI18nCatalog } from '../../../api/runtime-i18n';
import { useBlockI18n } from '../use-block-i18n';

vi.mock('../../../api/runtime-i18n', () => ({
  fetchFrontstageRuntimeI18nCatalog: vi.fn()
}));
const fetchCatalog = vi.mocked(fetchFrontstageRuntimeI18nCatalog);

beforeEach(async () => {
  fetchCatalog.mockReset();
  resetAuthStore();
  useAuthStore.setState({
    sessionStatus: 'authenticated',
    actor: {
      id: 'user-1',
      current_workspace_id: 'workspace-1'
    } as NonNullable<ReturnType<typeof useAuthStore.getState>['actor']>
  });
  await appI18n.changeLanguage('zh_Hans');
});
afterEach(() => resetAuthStore());

function wrapper({ children }: PropsWithChildren) {
  return (
    <I18nextProvider i18n={appI18n}>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </I18nextProvider>
  );
}
let client: QueryClient;
beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
afterEach(() => client.clear());
const catalog = (locale: string, Account: string) => ({
  locale,
  messages: { Account },
  catalog_revision: 1,
  digest: locale
});

test('loads once for multiple blocks and changes locale without exposing the previous catalog', async () => {
  fetchCatalog.mockResolvedValueOnce(catalog('zh_Hans', '账号'));
  let resolveEnglish!: (value: ReturnType<typeof catalog>) => void;
  const english = new Promise<ReturnType<typeof catalog>>((resolve) => {
    resolveEnglish = resolve;
  });
  fetchCatalog.mockImplementationOnce(() => english);
  const { result } = renderHook(
    () => [
      useBlockI18n('workspace-1', true),
      useBlockI18n('workspace-1', true)
    ],
    { wrapper }
  );
  await waitFor(() => expect(result.current[0].t('Account')).toBe('账号'));
  expect(fetchCatalog).toHaveBeenCalledTimes(1);
  await act(() => appI18n.changeLanguage('en_US'));
  expect(result.current[0].locale).toBe('en_US');
  expect(result.current[0].status).toBe('loading');
  expect(result.current[0].t('Account')).toBe('Account');
  await act(async () => resolveEnglish(catalog('en_US', 'Account')));
  await waitFor(() => expect(result.current[0].status).toBe('ready'));
  expect(fetchCatalog).toHaveBeenCalledTimes(2);
  await act(() => appI18n.changeLanguage('zh_Hans'));
  expect(result.current[0].t('Account')).toBe('账号');
  expect(fetchCatalog).toHaveBeenCalledTimes(2);
});

test('denied fetch becomes error and never uses another identity catalog', async () => {
  fetchCatalog.mockResolvedValueOnce(catalog('zh_Hans', '账号'));
  const { result } = renderHook(() => useBlockI18n('workspace-1', true), {
    wrapper
  });
  await waitFor(() => expect(result.current.status).toBe('ready'));
  fetchCatalog.mockRejectedValueOnce(new Error('Forbidden'));
  act(() =>
    useAuthStore.setState({
      actor: { ...useAuthStore.getState().actor!, id: 'user-2' }
    })
  );
  expect(result.current.t('Account')).toBe('Account');
  await waitFor(() => expect(result.current.status).toBe('error'));
  act(() => useAuthStore.getState().setAnonymous());
  expect(result.current.t('Account')).toBe('Account');
});

test('inactive or mismatched workspace does not fetch', () => {
  const { result, rerender } = renderHook(
    ({ workspace, active }) => useBlockI18n(workspace, active),
    {
      wrapper,
      initialProps: { workspace: 'workspace-1', active: false }
    }
  );
  rerender({ workspace: 'workspace-2', active: true });
  expect(fetchCatalog).not.toHaveBeenCalled();
  expect(result.current.status).toBe('loading');
});
