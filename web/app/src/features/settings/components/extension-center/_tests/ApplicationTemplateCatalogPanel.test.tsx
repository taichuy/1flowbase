import { StrictMode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  getSystemTemplateCatalog: vi.fn(),
  getApplicationTemplateCatalog: vi.fn(),
  downloadSystemTemplate: vi.fn(),
  previewSystemTemplate: vi.fn(),
  installSystemTemplate: vi.fn()
}));
const navigate = vi.hoisted(() => vi.fn());
vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  ...api
}));
vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  useNavigate: () => navigate
}));
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../../state/auth-store';
import { SettingsExtensionCenterSection } from '../../../pages/settings-page/SettingsExtensionCenterSection';

const body = { catalog_id: 'official/gateway-demo', release_version: 2 };
const downloadedPackage = {
  schema_version: '1flowbase.portable-template/v2',
  pages: [],
  applications: [],
  data_models: [],
  plugins: [],
  i18n_entries: [{ key: 'wiki.title', locale: 'en_US', translation: 'Wiki' }],
  release: { template_id: '@taichuy/wiki', release_version: 2 }
};
const preview = {
  valid: true,
  counts: {
    pages: 1,
    applications: 1,
    data_models: 2,
    mcp_instances: 0,
    i18n_entries: 0
  },
  failures: [],
  warnings: [],
  dependencies: [],
  effects: [],
  mcp_shared_tool_impacts: []
};
function setup(props: { cursor?: string; q?: string } = {}) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const view = (route: { cursor?: string; q?: string }) => (
    <StrictMode>
      <App>
        <QueryClientProvider client={client}>
          <SettingsExtensionCenterSection
            category="application-templates"
            {...route}
          />
        </QueryClientProvider>
      </App>
    </StrictMode>
  );
  const utils = render(view(props));
  return {
    ...utils,
    rerenderSection: (route: { cursor?: string; q?: string }) =>
      utils.rerender(view(route))
  };
}
async function openPreview() {
  fireEvent.click(
    await screen.findByRole('button', { name: 'Install template' })
  );
  await waitFor(() =>
    expect(api.previewSystemTemplate).toHaveBeenCalledWith(
      downloadedPackage,
      'csrf-token'
    )
  );
}
describe('application templates extension tab', () => {
  beforeEach(async () => {
    Object.values(api).forEach((mock) => mock.mockReset());
    navigate.mockReset();
    await appI18n.changeLanguage('en_US');
    useAuthStore.setState({
      csrfToken: 'csrf-token',
      actor: null,
      me: {
        permissions: [
          'system_templates.catalog',
          'system_templates.download',
          'system_templates.preview',
          'system_templates.install'
        ]
      } as NonNullable<ReturnType<typeof useAuthStore.getState>['me']>
    });
    api.getApplicationTemplateCatalog.mockResolvedValue({
      next_cursor: null,
      total: 1,
      application_templates: [
        {
          template_id: 'gateway-demo',
          release_version: 2,
          name: 'Gateway demo',
          description: 'Build a gateway workspace',
          checksum: 'next',
          installed_release_version: 1,
          installed_checksum: 'previous',
          catalog_id: body.catalog_id,
          source: 'official'
        }
      ]
    });
    api.downloadSystemTemplate.mockResolvedValue(downloadedPackage);
    api.previewSystemTemplate.mockResolvedValue(preview);
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [],
      updated: [],
      skipped: [],
      id_map: {},
      failures: []
    });
  });
  test('selects the dedicated tab, displays release state and manually installs the exact previewed version reference', async () => {
    setup();
    expect(
      await screen.findByRole('tab', { name: 'Application templates' })
    ).toHaveAttribute('aria-selected', 'true');
    expect(await screen.findByText('Gateway demo')).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: 'Installed version' })
    ).toBeInTheDocument();
    expect(screen.getByText('Build a gateway workspace')).toBeInTheDocument();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
    expect(api.getSystemTemplateCatalog).not.toHaveBeenCalled();
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalledWith({
      cursor: undefined,
      q: undefined
    });
    await openPreview();
    const install = await screen.findByRole('button', {
      name: 'Confirm installation'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(await screen.findByText('Template installed')).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(
      downloadedPackage,
      'csrf-token'
    );
    expect(api.downloadSystemTemplate).toHaveBeenCalledExactlyOnceWith(
      body,
      'csrf-token'
    );
    const completedInstall = screen.getByRole('button', {
      name: 'Confirm installation'
    });
    expect(completedInstall).toBeDisabled();
    fireEvent.click(completedInstall);
    expect(api.installSystemTemplate).toHaveBeenCalledTimes(1);
  });
  test('catalog updates preserve local changes through the shared preview and installation contract', async () => {
    const skipped = [
      {
        kind: 'page',
        source_id: 'customized-page',
        target_id: 'customized-page',
        reason: 'user_modified'
      }
    ];
    api.previewSystemTemplate.mockResolvedValue({
      ...preview,
      effects: skipped.map((item) => ({ ...item, action: 'skip' }))
    });
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [],
      updated: [],
      skipped,
      id_map: {},
      failures: []
    });
    setup();
    await openPreview();
    expect(
      await screen.findByText('Local changes are preserved.')
    ).toBeInTheDocument();
    const install = screen.getByRole('button', {
      name: 'Confirm installation'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(
      await screen.findByText(
        'Resources preserved and skipped: 1. Other eligible resources were processed.'
      )
    ).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(
      downloadedPackage,
      'csrf-token'
    );
    expect(api.downloadSystemTemplate).toHaveBeenCalledExactlyOnceWith(
      body,
      'csrf-token'
    );
  });
  test('invalid preview surfaces failures and blocks installation', async () => {
    api.previewSystemTemplate.mockResolvedValue({
      ...preview,
      valid: false,
      failures: ['Missing dependency']
    });
    setup();
    await openPreview();
    expect(await screen.findByText('Missing dependency')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Confirm installation' })
    ).toBeDisabled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('incomplete installation exposes backend failure and prevents blind repeat', async () => {
    api.installSystemTemplate.mockResolvedValue({
      complete: false,
      created: [],
      updated: [],
      skipped: [],
      id_map: {},
      failures: ['Plugin download failed']
    });
    setup();
    await openPreview();
    const install = screen.getByRole('button', {
      name: 'Confirm installation'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(
      await screen.findByText('Template installation incomplete')
    ).toBeInTheDocument();
    expect(screen.getByText('Plugin download failed')).toBeInTheDocument();
    const completedInstall = screen.getByRole('button', {
      name: 'Confirm installation'
    });
    expect(completedInstall).toBeDisabled();
    fireEvent.click(completedInstall);
    expect(api.installSystemTemplate).toHaveBeenCalledTimes(1);
  });
  test('preview request failure is visible and cannot install', async () => {
    api.previewSystemTemplate.mockRejectedValue(new Error('denied'));
    setup();
    await openPreview();
    expect(
      await screen.findByText(
        'Could not check installation changes. Close this dialog and try again.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Confirm installation' })
    ).toBeDisabled();
  });
  test('download failure cannot preview or install', async () => {
    api.downloadSystemTemplate.mockRejectedValue(
      new Error('upstream unavailable')
    );
    setup();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Install template' })
    );
    expect(
      await screen.findByText(
        'Could not download the template. Close this dialog and try again.'
      )
    ).toBeInTheDocument();
    expect(api.downloadSystemTemplate).toHaveBeenCalledExactlyOnceWith(
      body,
      'csrf-token'
    );
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('download progress blocks confirmation and closing discards the package', async () => {
    let finishDownload!: (value: typeof downloadedPackage) => void;
    api.downloadSystemTemplate.mockReturnValue(
      new Promise((resolve) => {
        finishDownload = resolve;
      })
    );
    setup();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Install template' })
    );
    expect(
      await screen.findByText('Downloading template…')
    ).toBeInTheDocument();
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
    expect(
      screen.queryByRole('button', { name: 'Confirm installation' })
    ).not.toBeInTheDocument();
    finishDownload(downloadedPackage);
    const confirm = await screen.findByRole('button', {
      name: 'Confirm installation'
    });
    await waitFor(() => expect(confirm).toBeEnabled());
    fireEvent.click(screen.getAllByRole('button', { name: 'Close' }).at(-1)!);
    await waitFor(() =>
      expect(
        screen.queryByRole('button', { name: 'Confirm installation' })
      ).not.toBeInTheDocument()
    );
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
    api.downloadSystemTemplate.mockResolvedValue(downloadedPackage);
    await openPreview();
    expect(api.downloadSystemTemplate).toHaveBeenCalledTimes(2);
  });
  test('readonly catalog grant keeps templates visible and installation disabled', async () => {
    useAuthStore.setState({
      me: { permissions: ['system_templates.catalog'] } as NonNullable<
        ReturnType<typeof useAuthStore.getState>['me']
      >
    });
    setup();
    expect(
      await screen.findByRole('button', { name: 'Install template' })
    ).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Upload template' })
    ).toBeDisabled();
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalled();
    expect(api.downloadSystemTemplate).not.toHaveBeenCalled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });

  test('missing permission prevents catalog and installation requests', async () => {
    useAuthStore.setState({ me: null });
    setup();
    expect(
      await screen.findByText(
        'System backup and template permission is required.'
      )
    ).toBeInTheDocument();
    expect(api.getApplicationTemplateCatalog).not.toHaveBeenCalled();
    expect(api.getSystemTemplateCatalog).not.toHaveBeenCalled();
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('missing csrf disables preview action', async () => {
    useAuthStore.setState({ csrfToken: null });
    setup();
    expect(
      await screen.findByRole('button', { name: 'Install template' })
    ).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Upload template' })
    ).toBeDisabled();
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
  });
  test('catalog failure remains visible with a retry', async () => {
    api.getApplicationTemplateCatalog.mockRejectedValue(
      new Error('unavailable')
    );
    setup();
    expect(
      await screen.findByText('Could not load application templates.')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Reload templates' })
    ).toBeEnabled();
  });
  test('keeps the URL query and appends the backend next cursor without page navigation', async () => {
    const first = {
      template_id: 'first',
      catalog_id: 'official/first',
      release_version: 1,
      name: 'First template',
      description: 'First description',
      checksum: 'first',
      installed_release_version: null,
      installed_checksum: null,
      source: 'official'
    };
    api.getApplicationTemplateCatalog
      .mockResolvedValueOnce({
        application_templates: [first],
        next_cursor: 'next',
        total: 2
      })
      .mockResolvedValueOnce({
        application_templates: [
          { ...first, catalog_id: 'official/second', name: 'Second template' }
        ],
        next_cursor: null,
        total: 2
      });
    const { container } = setup({ q: 'demo' });
    expect(await screen.findByText('First template')).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: 'Application templates' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Next page' })
    ).not.toBeInTheDocument();
    const scroll = container.querySelector('.ant-listy-holder')!;
    fireEvent.scroll(scroll);
    expect(await screen.findByText('Second template')).toBeInTheDocument();
    expect(screen.getByText('First template')).toBeInTheDocument();
    expect(api.getApplicationTemplateCatalog).toHaveBeenLastCalledWith({
      q: 'demo',
      cursor: 'next'
    });
    fireEvent.scroll(scroll);
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalledTimes(2);
    expect(navigate).not.toHaveBeenCalled();
  });
  test('search resets the cursor and replaces accumulated results', async () => {
    const view = setup({ q: 'old', cursor: 'old-cursor' });
    expect(await screen.findByText('Gateway demo')).toBeInTheDocument();
    const search = screen.getByRole('searchbox', {
      name: 'Search application templates'
    });
    fireEvent.change(search, { target: { value: '  gateway  ' } });
    fireEvent.keyDown(search, {
      key: 'Enter',
      code: 'Enter',
      charCode: 13,
      keyCode: 13
    });
    expect(navigate).toHaveBeenLastCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'application-templates' },
      search: { q: 'gateway', cursor: undefined }
    });
    api.getApplicationTemplateCatalog.mockResolvedValue({
      application_templates: [],
      next_cursor: null,
      total: 0
    });
    view.rerenderSection({ q: 'gateway' });
    expect(
      await screen.findByText('No application templates available.')
    ).toBeInTheDocument();
    expect(screen.queryByText('Gateway demo')).not.toBeInTheDocument();
    expect(api.getApplicationTemplateCatalog).toHaveBeenLastCalledWith({
      q: 'gateway',
      cursor: undefined
    });
  });
  test('preserves loaded rows on next-page failure and retries the failed cursor', async () => {
    const firstPage = await api.getApplicationTemplateCatalog();
    api.getApplicationTemplateCatalog.mockClear();
    api.getApplicationTemplateCatalog
      .mockResolvedValueOnce({ ...firstPage, next_cursor: 'next', total: 2 })
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({
        application_templates: [],
        next_cursor: null,
        total: 2
      });
    const { container } = setup();
    expect(await screen.findByText('Gateway demo')).toBeInTheDocument();
    const scroll = container.querySelector('.ant-listy-holder')!;
    fireEvent.scroll(scroll);
    expect(
      await screen.findByText('Could not load application templates.')
    ).toBeInTheDocument();
    expect(screen.getByText('Gateway demo')).toBeInTheDocument();
    fireEvent.scroll(scroll);
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole('button', { name: 'Reload templates' }));
    await waitFor(() =>
      expect(
        screen.queryByText('Could not load application templates.')
      ).not.toBeInTheDocument()
    );
    expect(api.getApplicationTemplateCatalog).toHaveBeenLastCalledWith({
      q: undefined,
      cursor: 'next'
    });
    expect(screen.getByText('Gateway demo')).toBeInTheDocument();
  });
  test('repeated scroll events do not duplicate an in-flight next page', async () => {
    const firstPage = await api.getApplicationTemplateCatalog();
    api.getApplicationTemplateCatalog.mockClear();
    let resolvePage!: (page: typeof firstPage) => void;
    api.getApplicationTemplateCatalog
      .mockResolvedValueOnce({ ...firstPage, next_cursor: 'next', total: 2 })
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolvePage = resolve;
          })
      );
    const { container } = setup();
    expect(await screen.findByText('Gateway demo')).toBeInTheDocument();
    const scroll = container.querySelector('.ant-listy-holder')!;
    fireEvent.scroll(scroll);
    fireEvent.scroll(scroll);
    fireEvent.scroll(scroll);
    await waitFor(() =>
      expect(api.getApplicationTemplateCatalog).toHaveBeenCalledTimes(2)
    );
    resolvePage({ application_templates: [], next_cursor: null, total: 1 });
    await waitFor(() =>
      expect(screen.getByRole('table')).toHaveAttribute('aria-busy', 'false')
    );
  });
  test.each(['json', 'zip'])(
    'uploads a local %s template through preview before installation',
    async (extension) => {
      const { container } = setup();
      expect(
        await screen.findByRole('button', { name: 'Upload template' })
      ).toBeEnabled();
      const content =
        extension === 'json'
          ? JSON.stringify(downloadedPackage)
          : 'zip-fixture';
      const file = new File([content], `template.${extension}`, {
        type: extension === 'json' ? 'application/json' : 'application/zip'
      });
      if (extension === 'json')
        Object.defineProperty(file, 'text', { value: async () => content });
      fireEvent.change(container.querySelector('input[type="file"]')!, {
        target: { files: [file] }
      });
      const packageBody =
        extension === 'json'
          ? downloadedPackage
          : { archive_base64: btoa(content) };
      await waitFor(() =>
        expect(api.previewSystemTemplate).toHaveBeenCalledWith(
          packageBody,
          'csrf-token'
        )
      );
      expect(api.installSystemTemplate).not.toHaveBeenCalled();
      expect(api.downloadSystemTemplate).not.toHaveBeenCalled();
      const confirm = within(
        await screen.findByRole('dialog', { name: 'Import template' })
      ).getByRole('button', { name: 'Install template' });
      await waitFor(() => expect(confirm).toBeEnabled());
      fireEvent.click(confirm);
      expect(await screen.findByText('Template installed')).toBeInTheDocument();
      expect(api.installSystemTemplate).toHaveBeenCalledWith(
        packageBody,
        'csrf-token'
      );
    }
  );
  test('an unreadable local file cannot preview or install', async () => {
    const { container } = setup();
    await screen.findByRole('button', { name: 'Upload template' });
    const file = new File(['invalid json'], 'invalid.json');
    Object.defineProperty(file, 'text', { value: async () => 'invalid json' });
    fireEvent.change(container.querySelector('input[type="file"]')!, {
      target: { files: [file] }
    });
    expect(
      await screen.findByText(
        'Could not read this template file. Choose a ZIP archive or a legacy JSON file.'
      )
    ).toBeInTheDocument();
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('upload requires preview, install and csrf grants', async () => {
    useAuthStore.setState({
      me: {
        permissions: ['system_templates.catalog', 'system_templates.install']
      } as NonNullable<ReturnType<typeof useAuthStore.getState>['me']>
    });
    const { container } = setup();
    expect(
      await screen.findByRole('button', { name: 'Upload template' })
    ).toBeDisabled();
    fireEvent.change(container.querySelector('input[type="file"]')!, {
      target: {
        files: [new File([JSON.stringify(downloadedPackage)], 'template.json')]
      }
    });
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
  });
});
