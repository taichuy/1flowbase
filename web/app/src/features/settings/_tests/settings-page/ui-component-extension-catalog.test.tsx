import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const routerApi = vi.hoisted(() => ({ navigate: vi.fn() }));
const uiManagementApi = vi.hoisted(() => ({
  settingsUiComponentsQueryKey: ['settings', 'ui-management', 'components'],
  fetchSettingsUiCatalogPage: vi.fn(),
  searchSettingsUiCatalog: vi.fn(),
  fetchSettingsUiCatalogUpdateStatus: vi.fn(),
  downloadSettingsUiCatalogComponent: vi.fn(),
  syncSettingsUiCatalogGroup: vi.fn()
}));

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => routerApi.navigate
}));
vi.mock('../../api/ui-management', () => uiManagementApi);

import { AppI18nProvider } from '../../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { UiComponentCatalogPanel } from '../../components/extension-center/UiComponentCatalogPanel';

const catalogComponent = {
  component_code: 'taichuy.ant-design-x.bubble',
  name: 'Bubble',
  description: 'Conversation bubble',
  import_code: "import { Bubble } from '@ant-design/x';",
  source_code: '<Bubble content="Hello" />',
  source: 'taichuy',
  group: 'ant-design-x',
  upstream: { identity: '@ant-design/x', version: '2.9.0' },
  version: '1.0.0',
  keywords: ['chat'],
  catalog_updated_at: '2026-08-23T00:00:00Z',
  source_locator: 'ui_components/@taichuy/ant-design-x/bubble.json',
  source_checksum: `sha256:${'a'.repeat(64)}`
};

function renderPanel() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <App>
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </App>
    </AppI18nProvider>
  );
  return render(<UiComponentCatalogPanel canManage />, { wrapper });
}

describe('Extension Center UI component catalog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    useAuthStore.setState({ csrfToken: 'csrf' });
    uiManagementApi.fetchSettingsUiCatalogPage.mockResolvedValue({
      catalog_version: '1.0.0',
      total_components: 1,
      page_size: 100,
      page: 1,
      cursor: 'start',
      next_cursor: null,
      records: [catalogComponent]
    });
    uiManagementApi.searchSettingsUiCatalog.mockResolvedValue({
      catalog_version: '1.0.0',
      page: 1,
      page_size: 20,
      total_entries: 1,
      entries: [{ ...catalogComponent, catalog_page: 1 }]
    });
    uiManagementApi.fetchSettingsUiCatalogUpdateStatus.mockResolvedValue({
      catalog_version: '1.0.0',
      source_fingerprint: `sha256:${'b'.repeat(64)}`,
      update_available: true,
      groups: [
        {
          source: 'taichuy',
          group: 'ant-design-x',
          remote_records: 2,
          new_or_updated_records: 1,
          removed_records: 0,
          update_available: true
        }
      ]
    });
    uiManagementApi.downloadSettingsUiCatalogComponent.mockResolvedValue({});
    uiManagementApi.syncSettingsUiCatalogGroup.mockResolvedValue({
      synchronized_records: 2
    });
  });

  test('discovers catalog records and uses the existing download and group sync actions', async () => {
    renderPanel();

    expect(await screen.findByText('Bubble')).toBeInTheDocument();
    expect(screen.getByText('taichuy / ant-design-x')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'UI 组件' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(screen.getByRole('link', { name: '前往 UI 管理' })).toHaveAttribute(
      'href',
      '/settings/ui-management/components'
    );
    expect(
      screen.queryByRole('button', { name: /安装|启用|停用|卸载/ })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '下载' }));
    await waitFor(() =>
      expect(
        uiManagementApi.downloadSettingsUiCatalogComponent
      ).toHaveBeenCalledWith('taichuy.ant-design-x.bubble', 'csrf')
    );

    fireEvent.click(screen.getByRole('button', { name: '同步分组' }));
    await waitFor(() =>
      expect(uiManagementApi.syncSettingsUiCatalogGroup).toHaveBeenCalledWith(
        'taichuy',
        'ant-design-x',
        'csrf'
      )
    );
  });

  test('navigates to another extension center category through the shared tab contract', async () => {
    renderPanel();
    await screen.findByText('Bubble');

    fireEvent.click(screen.getByRole('tab', { name: 'runtime-extensions' }));
    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { q: undefined, cursor: undefined }
    });
  });
});
