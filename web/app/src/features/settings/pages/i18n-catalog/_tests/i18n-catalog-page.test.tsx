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

const officialEntry = {
  module: '@1flowbase/common',
  msgid: 'Settings',
  locale: 'zh_Hans',
  official_translation: '设置',
  override_translation: '系统设置',
  custom_translation: null,
  effective_value: '系统设置',
  origin: 'official_override' as const,
  missing: false,
  obsolete: true,
  revision: 8
};

const customEntry = {
  module: 'workspace/custom',
  msgid: 'Greeting',
  locale: 'zh_Hans',
  official_translation: null,
  override_translation: null,
  custom_translation: '欢迎',
  effective_value: '欢迎',
  origin: 'custom' as const,
  missing: false,
  obsolete: false,
  revision: 8
};

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

describe('I18nCatalogPage batch fixtures', () => {
  beforeEach(() => {
    resetAuthStore();
    authenticate();
    vi.clearAllMocks();
    catalogApi.fetchSettingsI18nCatalogEntries.mockResolvedValue({
      entries: [officialEntry, customEntry],
      total: 2,
      revision: 8
    });
    catalogApi.fetchSettingsI18nCatalogEntry.mockImplementation(
      async (identity) =>
        identity.msgid === 'Greeting' ? customEntry : officialEntry
    );
    catalogApi.saveSettingsI18nCatalogOverride.mockResolvedValue({
      revision: 9,
      entry: officialEntry
    });
    catalogApi.saveSettingsCustomI18nCatalogTranslation.mockResolvedValue({
      revision: 9,
      entry: customEntry
    });
    catalogApi.restoreSettingsI18nCatalogOverride.mockResolvedValue({
      revision: 9,
      entry: officialEntry
    });
    catalogApi.deleteSettingsCustomI18nCatalogKey.mockResolvedValue({
      revision: 9
    });
    catalogApi.restoreAllSettingsI18nCatalogOverrides.mockResolvedValue({
      revision: 9
    });
  });

  test('AC-007 renders compact desktop and honest mobile browse contracts from real entries', async () => {
    renderPage();

    expect((await screen.findAllByText('系统设置')).length).toBeGreaterThan(0);
    expect(
      screen.getByTestId('i18n-catalog-desktop-table')
    ).toBeInTheDocument();
    expect(screen.getByTestId('i18n-catalog-mobile-list')).toBeInTheDocument();
    expect(screen.getAllByText('官方覆盖值').length).toBeGreaterThan(0);
    expect(screen.getAllByText('过期翻译').length).toBeGreaterThan(0);
  });

  test('AC-008 sends search, module, locale and origin filters to the list query', async () => {
    renderPage();
    await screen.findAllByText('系统设置');

    fireEvent.change(screen.getByPlaceholderText('搜索消息标识或翻译'), {
      target: { value: 'Settings' }
    });
    fireEvent.change(screen.getByPlaceholderText('翻译模块'), {
      target: { value: '@1flowbase/common' }
    });
    fireEvent.mouseDown(screen.getByTestId('i18n-catalog-locale-filter'));
    fireEvent.click(await screen.findByRole('option', { name: 'zh_Hans' }));
    fireEvent.mouseDown(screen.getByTestId('i18n-catalog-origin-filter'));
    fireEvent.click(await screen.findByRole('option', { name: '官方覆盖值' }));
    fireEvent.click(screen.getByRole('button', { name: '应用翻译筛选' }));

    await waitFor(() =>
      expect(
        catalogApi.fetchSettingsI18nCatalogEntries
      ).toHaveBeenLastCalledWith(
        expect.objectContaining({
          search: 'Settings',
          module: '@1flowbase/common',
          locale: 'zh_Hans',
          origin: 'official_override',
          offset: 0,
          limit: 20
        })
      )
    );
  });

  test('AC-008 opens all source layers and saves with the selected entry revision', async () => {
    renderPage();
    const desktopTable = await screen.findByTestId(
      'i18n-catalog-desktop-table'
    );
    fireEvent.click(within(desktopTable).getByText('系统设置'));

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
    const desktopTable = await screen.findByTestId(
      'i18n-catalog-desktop-table'
    );
    fireEvent.click(within(desktopTable).getByText('系统设置'));
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
    await screen.findAllByText('系统设置');

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

    const desktopTable = screen.getByTestId('i18n-catalog-desktop-table');
    fireEvent.click(within(desktopTable).getByText('欢迎'));
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
