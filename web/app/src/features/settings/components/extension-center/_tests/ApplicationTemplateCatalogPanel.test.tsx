import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  getSystemTemplateCatalog: vi.fn(),
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

const body = {
  schema_version: '1flowbase.portable-template/v1',
  pages: [],
  release: { template_id: 'gateway-demo', release_version: 2 }
};
const preview = {
  valid: true,
  counts: { pages: 1, applications: 1, data_models: 2, mcp_instances: 0 },
  failures: [],
  warnings: [],
  dependencies: [],
  effects: [],
  mcp_shared_tool_impacts: []
};
function setup() {
  return render(
    <App>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: {
              queries: { retry: false },
              mutations: { retry: false }
            }
          })
        }
      >
        <SettingsExtensionCenterSection category="application-templates" />
      </QueryClientProvider>
    </App>
  );
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
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: [],
      mcp_instances: [],
      application_templates: [
        {
          template_id: 'gateway-demo',
          release_version: 2,
          name: 'Gateway demo',
          description: 'Build a gateway workspace',
          checksum: 'next',
          installed_release_version: 1,
          installed_checksum: 'previous',
          package: body
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
  test('selects the dedicated tab, displays release state and manually installs the exact previewed package', async () => {
    setup();
    expect(
      await screen.findByRole('tab', { name: 'Application templates' })
    ).toHaveAttribute('aria-selected', 'true');
    expect(await screen.findByText('Gateway demo')).toBeInTheDocument();
    expect(screen.getByText('Installed version')).toBeInTheDocument();
    expect(screen.getByText('Build a gateway workspace')).toBeInTheDocument();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
    await openPreview();
    const install = await screen.findByRole('button', {
      name: 'Confirm install and overwrite'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(await screen.findByText('Template installed')).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token');
    expect(install).toBeDisabled();
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
    expect(install).toBeDisabled();
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
    api.getSystemTemplateCatalog.mockRejectedValue(new Error('unavailable'));
    setup();
    expect(
      await screen.findByText('Could not load application templates.')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Reload templates' })
    ).toBeEnabled();
  });
});
