import { ApiClientError } from '@1flowbase/api-client';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const catalogApi = vi.hoisted(() => ({
  settingsI18nCatalogQueryKey: ['settings', 'i18n-catalog'] as const,
  settingsI18nCatalogListQueryKey: vi.fn((request) => [
    'settings',
    'i18n-catalog',
    'list',
    request
  ]),
  settingsI18nCatalogEntryQueryKey: vi.fn((request) => [
    'settings',
    'i18n-catalog',
    'entry',
    request
  ]),
  fetchSettingsI18nCatalogEntries: vi.fn(),
  fetchSettingsI18nCatalogEntry: vi.fn(),
  saveSettingsI18nCatalogOverride: vi.fn(),
  saveSettingsCustomI18nCatalogTranslation: vi.fn(),
  restoreSettingsI18nCatalogOverride: vi.fn(),
  deleteSettingsCustomI18nCatalogKey: vi.fn(),
  restoreAllSettingsI18nCatalogOverrides: vi.fn()
}));

vi.mock('../../../api/i18n-catalog', () => catalogApi);

import { AppProviders } from '../../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { I18nCatalogPage } from '../I18nCatalogPage';
import {
  createSettingsI18nCatalogTestServer,
  settingsI18nCatalogTestLocales
} from './i18n-catalog-test-fixture';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'root-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'root-1',
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

function renderPage() {
  return render(
    <AppProviders>
      <I18nCatalogPage />
    </AppProviders>
  );
}

async function findLoadedDesktopEntry(value: string) {
  const desktopTable = screen.getByTestId('i18n-catalog-desktop-table');
  const entry = await within(desktopTable).findByText(value);

  await waitFor(() =>
    expect(
      desktopTable.querySelector('.ant-spin-spinning')
    ).not.toBeInTheDocument()
  );
  return entry;
}

describe('I18nCatalogPage batch fixtures', () => {
  beforeEach(() => {
    const catalogServer = createSettingsI18nCatalogTestServer();

    resetAuthStore();
    authenticate();
    vi.clearAllMocks();
    catalogApi.fetchSettingsI18nCatalogEntries.mockImplementation(
      catalogServer.listEntries
    );
    catalogApi.fetchSettingsI18nCatalogEntry.mockImplementation(
      catalogServer.getEntry
    );
    catalogApi.saveSettingsI18nCatalogOverride.mockImplementation(
      catalogServer.saveOverride
    );
    catalogApi.saveSettingsCustomI18nCatalogTranslation.mockImplementation(
      catalogServer.saveCustomTranslation
    );
    catalogApi.restoreSettingsI18nCatalogOverride.mockImplementation(
      catalogServer.restoreOverride
    );
    catalogApi.deleteSettingsCustomI18nCatalogKey.mockImplementation(
      catalogServer.deleteCustomKey
    );
    catalogApi.restoreAllSettingsI18nCatalogOverrides.mockImplementation(
      catalogServer.restoreAllOverrides
    );
  });

  test('AC-007 renders compact desktop and honest mobile browse contracts from real entries', async () => {
    renderPage();

    expect(settingsI18nCatalogTestLocales).toEqual(['en_US', 'zh_Hans']);
    await findLoadedDesktopEntry('系统设置');
    expect(
      screen.getByTestId('i18n-catalog-desktop-table')
    ).toBeInTheDocument();
    expect(screen.getByTestId('i18n-catalog-mobile-list')).toBeInTheDocument();
    expect(screen.getAllByText('官方覆盖值').length).toBeGreaterThan(0);
    expect(screen.getAllByText('过期翻译').length).toBeGreaterThan(0);
  });

  test('AC-008 opens all source layers and saves with the selected entry revision', async () => {
    renderPage();
    fireEvent.click(await findLoadedDesktopEntry('系统设置'));

    const drawer = await screen.findByTestId('i18n-catalog-entry-drawer');
    await within(drawer).findByLabelText('覆盖翻译');
    expect(within(drawer).getByText('设置')).toBeInTheDocument();
    fireEvent.change(within(drawer).getByLabelText('覆盖翻译'), {
      target: { value: '新设置' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: '保存翻译' }));

    await waitFor(() =>
      expect(catalogApi.saveSettingsI18nCatalogOverride).toHaveBeenCalledWith(
        expect.objectContaining({
          translation: '新设置',
          expected_revision: 8
        }),
        'csrf-123'
      )
    );
    expect(
      within(drawer).queryByRole('button', { name: '删除自定义翻译键' })
    ).not.toBeInTheDocument();
  });

  test('AC-009 reports revision conflicts and refetches instead of overwriting silently', async () => {
    catalogApi.saveSettingsI18nCatalogOverride.mockRejectedValueOnce(
      new ApiClientError({ status: 409, message: 'revision conflict' })
    );
    renderPage();
    fireEvent.click(await findLoadedDesktopEntry('系统设置'));
    const drawer = await screen.findByTestId('i18n-catalog-entry-drawer');
    await within(drawer).findByLabelText('覆盖翻译');
    fireEvent.click(within(drawer).getByRole('button', { name: '保存翻译' }));

    expect(
      await screen.findByTestId('i18n-catalog-conflict')
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(
        catalogApi.fetchSettingsI18nCatalogEntries.mock.calls.length
      ).toBeGreaterThan(1)
    );
  });

  test('AC-013 keeps global restore and custom deletion as distinct confirmed actions', async () => {
    renderPage();
    await findLoadedDesktopEntry('系统设置');

    fireEvent.click(screen.getByRole('button', { name: /恢复全部官方翻译/ }));
    const restoreDialog = await screen.findByTestId(
      'i18n-catalog-restore-all-confirmation'
    );
    expect(
      within(restoreDialog).getByText(/自定义键及其翻译会保留/)
    ).toBeInTheDocument();
    fireEvent.click(
      within(restoreDialog).getByRole('button', { name: '恢复翻译' })
    );
    await waitFor(() =>
      expect(
        catalogApi.restoreAllSettingsI18nCatalogOverrides
      ).toHaveBeenCalledWith({ expected_revision: 8 }, 'csrf-123')
    );
    expect(await screen.findByText('5 条翻译 · 修订 9')).toBeInTheDocument();

    fireEvent.click(await findLoadedDesktopEntry('欢迎'));
    const drawer = await screen.findByTestId('i18n-catalog-entry-drawer');
    fireEvent.click(
      await within(drawer).findByRole('button', {
        name: '删除自定义翻译键'
      })
    );
    expect(
      await screen.findByTestId('i18n-catalog-delete-confirmation')
    ).toBeInTheDocument();
  });
});
