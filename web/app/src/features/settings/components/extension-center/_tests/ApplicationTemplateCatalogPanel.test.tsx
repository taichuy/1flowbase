import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  getSystemTemplateCatalog: vi.fn(),
  getApplicationTemplateCatalog: vi.fn(),
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
const preview = {
  valid: true,
  counts: { pages: 1, applications: 1, data_models: 2, mcp_instances: 0 },
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
    <App>
      <QueryClientProvider client={client}>
        <SettingsExtensionCenterSection
          category="application-templates"
          {...route}
        />
      </QueryClientProvider>
    </App>
  );
  const result = render(view(props));
  return {
    ...result,
    rerenderSection: (route: { cursor?: string; q?: string }) =>
      result.rerender(view(route))
  };
}
async function openPreview() {
  fireEvent.click(
    await screen.findByRole('button', { name: 'Preview installation' })
  );
  await waitFor(() =>
    expect(api.previewSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token')
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
        permissions: ['settings_feature.access.system.backups']
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
    api.previewSystemTemplate.mockResolvedValue(preview);
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [],
      updated: [],
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
      name: 'Confirm install and overwrite'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(await screen.findByText('Template installed')).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token');
    const completedInstall = screen.getByRole('button', {
      name: 'Confirm install and overwrite'
    });
    expect(completedInstall).toBeDisabled();
    fireEvent.click(completedInstall);
    expect(api.installSystemTemplate).toHaveBeenCalledTimes(1);
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
      screen.getByRole('button', { name: 'Confirm install and overwrite' })
    ).toBeDisabled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('incomplete installation exposes backend failure and prevents blind repeat', async () => {
    api.installSystemTemplate.mockResolvedValue({
      complete: false,
      created: [],
      updated: [],
      id_map: {},
      failures: ['Plugin download failed']
    });
    setup();
    await openPreview();
    const install = screen.getByRole('button', {
      name: 'Confirm install and overwrite'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(
      await screen.findByText('Template installation incomplete')
    ).toBeInTheDocument();
    expect(screen.getByText('Plugin download failed')).toBeInTheDocument();
    const completedInstall = screen.getByRole('button', {
      name: 'Confirm install and overwrite'
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
        'Template preview failed. Close this preview and try again.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Confirm install and overwrite' })
    ).toBeDisabled();
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
      await screen.findByRole('button', { name: 'Preview installation' })
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
  test('fetches the URL cursor and query and navigates to the backend next cursor', async () => {
    api.getApplicationTemplateCatalog.mockResolvedValue({
      application_templates: [
        {
          template_id: 'second',
          catalog_id: 'official/second',
          release_version: 3,
          name: 'Second page template',
          description: 'Only on the second page',
          checksum: 'second',
          installed_release_version: null,
          installed_checksum: null,
          source: 'official'
        }
      ],
      next_cursor: 'third-cursor',
      total: 37
    });
    setup({ cursor: 'second-cursor', q: 'gateway' });
    expect(await screen.findByText('Second page template')).toBeInTheDocument();
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalledWith({
      cursor: 'second-cursor',
      q: 'gateway'
    });
    expect(screen.queryByText('Gateway demo')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(navigate).toHaveBeenLastCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'application-templates' },
      search: { q: 'gateway', cursor: 'third-cursor' }
    });
  });
  test('search resets pagination and an empty last page cannot advance', async () => {
    api.getApplicationTemplateCatalog.mockResolvedValue({
      application_templates: [],
      next_cursor: null,
      total: 0
    });
    setup({ cursor: 'old-cursor', q: 'old' });
    expect(
      await screen.findByText('No application templates available.')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Next page' })).toBeDisabled();
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
    expect(api.previewSystemTemplate).not.toHaveBeenCalled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('replaces server pages and returns to the previously visited cursor', async () => {
    api.getApplicationTemplateCatalog.mockImplementation(
      async ({ cursor }) => ({
        application_templates: [
          {
            template_id: cursor ?? 'first',
            catalog_id: cursor ?? 'first',
            release_version: 1,
            name: cursor ?? 'First page template',
            description: 'Page metadata',
            checksum: 'digest',
            installed_release_version: null,
            installed_checksum: null,
            source: 'official'
          }
        ],
        next_cursor:
          cursor === 'page-three' ? null : cursor ? 'page-three' : 'page-two',
        total: 3
      })
    );
    const view = setup({ q: 'demo' });
    expect(await screen.findByText('First page template')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    view.rerenderSection({ q: 'demo', cursor: 'page-two' });
    expect(await screen.findByText('page-two')).toBeInTheDocument();
    expect(screen.queryByText('First page template')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    view.rerenderSection({ q: 'demo', cursor: 'page-three' });
    expect(await screen.findByText('page-three')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Previous page' }));
    expect(navigate).toHaveBeenLastCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'application-templates' },
      search: { q: 'demo', cursor: 'page-two' }
    });
    expect(api.getApplicationTemplateCatalog).toHaveBeenCalledWith({
      q: 'demo',
      cursor: 'page-three'
    });
  });
});
