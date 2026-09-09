import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  render,
  screen,
  waitFor,
  cleanup,
  fireEvent,
  within
} from '@testing-library/react';
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  test,
  vi
} from 'vitest';

const api = vi.hoisted(() => ({
  fetchConsolePluginSettingsPage: vi.fn(),
  fetchConsoleUiTemplates: vi.fn(),
  updateConsoleUiTemplate: vi.fn(),
  listConsoleInstalledExtensions: vi.fn(),
  enableConsoleInstalledExtension: vi.fn()
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
// Replace the worker transport and module inventory; the actual compiler,
// preparation, evaluation and production portal execute the published source.
vi.mock(
  '../../../../shared/code-block/native-react-compiler-browser',
  async () => {
    const { compileNativeReactComponent } =
      await import('@1flowbase/page-runtime');
    return {
      compileNativeReactComponentInBrowser: async ({
        source,
        moduleDefinitions
      }: {
        source: string;
        moduleDefinitions: unknown;
      }) => compileNativeReactComponent(source, moduleDefinitions)
    };
  }
);
vi.mock('../../../frontstage/lib/native-modules/registry', async () => {
  const { createNativeReactModuleRegistry } =
    await import('@1flowbase/page-runtime');
  const jsxRuntime = await import('react/jsx-runtime');
  return {
    FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS: [
      { module_source: 'react/jsx-runtime', exports: Object.keys(jsxRuntime) }
    ],
    createFrontstageNativeReactModuleRegistry: () =>
      createNativeReactModuleRegistry([
        {
          module_source: 'react/jsx-runtime',
          exports: Object.keys(jsxRuntime),
          load: async () => ({ module: jsxRuntime })
        }
      ])
  };
});

vi.mock('@tanstack/react-router', async (original) => ({
  ...(await original<typeof import('@tanstack/react-router')>()),
  useNavigate: () => vi.fn()
}));
vi.mock('../../../frontstage/hooks/use-frontstage-block-catalog', () => ({
  useFrontstageBlockCatalog: () => ({ items: [], loading: false })
}));
vi.mock(
  '../../../frontstage/components/jsx-studio/JsxStudioResourcePanel',
  () => ({ JsxStudioResourcePanel: () => null })
);
vi.mock('../../../frontstage/components/jsx-studio/JsxStudioRunPanel', () => ({
  JsxStudioRunPanel: () => null
}));
vi.mock('../../../../shared/code-block/BlockSourceStudio', () => ({
  BlockSourceStudio: (props: {
    editorHeader?: import('react').ReactNode;
    source: string;
    onChange(source: string): void;
    onSave(): void;
    renderResource(section: 'configuration'): import('react').ReactNode;
  }) => (
    <section>
      {props.editorHeader}
      {props.renderResource('configuration')}
      <textarea
        aria-label="Template source"
        value={props.source}
        onChange={(event) => props.onChange(event.target.value)}
      />
      <button onClick={props.onSave}>Save source</button>
    </section>
  )
}));

import { App } from 'antd';
import {
  appI18n as i18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { CodeTemplatesTab } from '../../components/ui-management/CodeTemplatesTab';
import { SettingsExtensionCenterSection } from '../../pages/settings-page/SettingsExtensionCenterSection';
import { PluginSettingsPage } from '../../pages/settings-page/PluginSettingsPage';
import { settingsSectionItemsFromConsoleNavigation } from '../../lib/settings-sections';

const page = {
  route_id: 'plugin.acme.preferences',
  feature_id: 'acme.preferences',
  template_id: 'template-1',
  provider_code: 'acme',
  contribution_code: 'preferences',
  source:
    'export default function Settings() { return <h1>Published preferences</h1>; }',
  language: 'tsx',
  revision: 2,
  applied_plugin_version: '1.0.0',
  overwrite_on_plugin_upgrade: true
};
function renderSurface(children: import('react').ReactNode) {
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
        {children}
      </QueryClientProvider>
    </App>
  );
}
function renderPage(route_id = page.route_id) {
  return renderSurface(<PluginSettingsPage route_id={route_id} />);
}

describe('AC-009/012 plugin settings published page', () => {
  beforeAll(loadApplicationI18nResources);
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('en_US');
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
  });
  afterEach(() => {
    cleanup();
    resetAuthStore();
  });
  test('retains the registered route identity rather than deriving plugin ownership from the URL', () => {
    expect(
      settingsSectionItemsFromConsoleNavigation({
        route_definitions: [
          {
            route_id: page.route_id,
            surface_key: 'unrelated',
            path: '/settings/preferences'
          }
        ],
        navigation_items: [
          {
            route_id: page.route_id,
            parent_item_id: 'settings',
            label_key: 'preferences',
            navigation_slot: 'settings',
            order: 1
          }
        ]
      })
    ).toEqual([
      {
        route_id: page.route_id,
        key: 'preferences',
        label_key: 'preferences',
        to: '/settings/preferences'
      }
    ]);
  });
  test('compiles and renders the authorized published source through the actual portal', async () => {
    api.fetchConsolePluginSettingsPage.mockResolvedValue(page);
    renderPage();
    await waitFor(() => {
      expect(
        screen.getByTestId('plugin-settings-page').shadowRoot?.textContent
      ).toContain('Published preferences');
    });
    expect(api.fetchConsolePluginSettingsPage).toHaveBeenCalledWith(
      page.route_id
    );
  });
  test('does not let a late response from a previous route replace the current page', async () => {
    let resolvePrevious!: (value: typeof page) => void;
    api.fetchConsolePluginSettingsPage.mockImplementation((route_id) =>
      route_id === 'previous'
        ? new Promise((resolve) => {
            resolvePrevious = resolve;
          })
        : Promise.resolve(page)
    );
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const view = render(
      <App>
        <QueryClientProvider client={client}>
          <PluginSettingsPage route_id="previous" />
        </QueryClientProvider>
      </App>
    );
    await waitFor(() =>
      expect(api.fetchConsolePluginSettingsPage).toHaveBeenCalledWith(
        'previous'
      )
    );
    view.rerender(
      <App>
        <QueryClientProvider client={client}>
          <PluginSettingsPage route_id={page.route_id} />
        </QueryClientProvider>
      </App>
    );
    await waitFor(() =>
      expect(
        screen.getByTestId('plugin-settings-page').shadowRoot?.textContent
      ).toContain('Published preferences')
    );
    resolvePrevious({
      ...page,
      route_id: 'previous',
      source: 'export default () => <p>Old route</p>'
    });
    await waitFor(() =>
      expect(
        client.getQueryData(['settings', 'plugin-settings-page', 'previous'])
      ).toBeDefined()
    );
    expect(
      screen.getByTestId('plugin-settings-page').shadowRoot?.textContent
    ).toContain('Published preferences');
    expect(
      screen.getByTestId('plugin-settings-page').shadowRoot?.textContent
    ).not.toContain('Old route');
  });
  test('reports invalid published source as an error rather than exposing source or diagnostics', async () => {
    api.fetchConsolePluginSettingsPage.mockResolvedValue({
      ...page,
      source: 'export default function broken({'
    });
    renderPage();
    await screen.findByRole('alert');
    expect(screen.queryByTestId('plugin-settings-page')).toBeNull();
    expect(screen.queryByText('export default function broken({')).toBeNull();
  });
  test.each([403, 404, 409])(
    'does not render a template when the page read rejects with %s',
    async (status) => {
      api.fetchConsolePluginSettingsPage.mockRejectedValue({ status });
      renderPage();
      await screen.findByRole('alert');
      expect(screen.queryByTestId('plugin-settings-page')).toBeNull();
    }
  );
});

describe('AC-012/014 template editor and formal activation entry', () => {
  beforeAll(loadApplicationI18nResources);
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('en_US');
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
  });
  afterEach(() => {
    cleanup();
    resetAuthStore();
  });

  test.each(['en_US', 'zh_Hans'])(
    'warns in %s before editing plugin-owned source and preserves the save contract',
    async (locale) => {
      await i18n.changeLanguage(locale);
      api.fetchConsoleUiTemplates.mockResolvedValue({
        official: [],
        default_template: null,
        managed: [
          {
            id: 'template-1',
            provider_code: 'acme',
            contribution_code: 'preferences',
            name: 'Acme preferences',
            latest_revision: {
              revision: 2,
              source: page.source,
              language: 'tsx',
              is_published: true
            },
            published_revision: {
              revision: 2,
              source: page.source,
              language: 'tsx',
              is_published: true
            },
            is_default: false,
            is_archived: false,
            owner_plugin_code: 'acme',
            owner_feature_id: 'acme.preferences',
            applied_plugin_version: '1.0.0',
            overwrite_on_plugin_upgrade: true
          }
        ]
      });
      api.updateConsoleUiTemplate.mockResolvedValue({ id: 'template-1' });
      renderSurface(<CodeTemplatesTab canManage />);
      const row = await screen.findByRole('row', { name: /Acme preferences/ });
      fireEvent.click(
        within(row).getByRole('button', {
          name: locale === 'en_US' ? 'Edit' : '编辑',
          exact: true
        })
      );
      const warning = await screen.findByTestId(
        'plugin-settings-overwrite-warning'
      );
      expect(warning.textContent).toContain(
        locale === 'en_US' ? 'overwriting your edits' : '覆盖您的修改'
      );
      expect(warning.textContent).toContain(
        locale === 'en_US' ? 're-enabling the same version' : '同版本重新启用'
      );
      expect(warning.textContent).toContain('acme.preferences');
      expect(warning.textContent).toContain('1.0.0');
      fireEvent.change(
        screen.getByRole('textbox', { name: 'Template source' }),
        { target: { value: 'export default () => <p>My edit</p>' } }
      );
      fireEvent.click(screen.getByRole('button', { name: 'Save source' }));
      await waitFor(() =>
        expect(api.updateConsoleUiTemplate).toHaveBeenCalledWith(
          'template-1',
          {
            name: 'Acme preferences',
            source: 'export default () => <p>My edit</p>',
            language: 'tsx'
          },
          'csrf-123'
        )
      );
    }
  );

  test.each(['en_US', 'zh_Hans'])(
    'selects the exact installed native version from its %s detail action',
    async (locale) => {
      await i18n.changeLanguage(locale);
      const versions = [
        { id: 'installation-1', version: '1.0.0', is_current: true },
        { id: 'installation-2', version: '2.0.0', is_current: false }
      ].map((version) => ({
        ...version,
        source_kind: 'local',
        trust_level: 'trusted_host',
        signature_status: 'valid',
        deletable: false,
        delete_reasons: ['retained'],
        local_checksum: null,
        expected_checksum: null,
        local_path: null
      }));
      const installed = {
        id: 'installation-1',
        category: 'host-extensions',
        catalog_id: 'host-extensions:acme/preferences',
        organization: 'acme',
        artifact_id: 'preferences',
        version: '1.0.0',
        node_id: 'node-1',
        source_kind: 'local',
        trust_level: 'trusted_host',
        warnings: [],
        status: 'installed',
        is_current: true,
        desired_state: 'active_requested',
        runtime_status: 'active',
        availability_status: 'available',
        application_action: 'none',
        application_status: 'not_required',
        installed_versions: versions,
        created_by: 'user-1',
        created_at: '',
        updated_at: ''
      };
      api.listConsoleInstalledExtensions.mockResolvedValue({
        entries: [installed],
        total_entries: 1,
        limit: 20,
        next_cursor: null
      });
      api.enableConsoleInstalledExtension.mockImplementation(
        async (installationId) => {
          if (installationId !== 'installation-2')
            throw new Error('Unexpected target installation');
          api.listConsoleInstalledExtensions.mockResolvedValue({
            entries: [
              {
                ...installed,
                id: 'installation-2',
                version: '2.0.0',
                runtime_status: 'inactive',
                availability_status: 'pending_restart',
                installed_versions: versions.map((version) => ({
                  ...version,
                  is_current: version.id === 'installation-2'
                }))
              }
            ],
            total_entries: 1,
            limit: 20,
            next_cursor: null
          });
          return { id: 'enable-task', status: 'completed' };
        }
      );
      renderSurface(<SettingsExtensionCenterSection category="installed" />);
      const row = await screen.findByRole('row', { name: /preferences/ });
      fireEvent.click(
        within(row).getByRole('button', {
          name: locale === 'en_US' ? 'View' : '查看',
          exact: true
        })
      );
      const label = locale === 'en_US' ? 'Select this version' : '选择此版本';
      const currentVersion = document.querySelector(
        'li[data-installation-id="installation-1"]'
      )! as HTMLElement;
      const nextVersion = document.querySelector(
        'li[data-installation-id="installation-2"]'
      )! as HTMLElement;
      expect(
        within(currentVersion).getByRole('button', { name: label })
      ).toBeDisabled();
      fireEvent.click(within(nextVersion).getByRole('button', { name: label }));
      await waitFor(() =>
        expect(api.enableConsoleInstalledExtension).toHaveBeenCalledWith(
          'installation-2',
          'csrf-123'
        )
      );
      await waitFor(() =>
        expect(
          screen.getByTestId('plugin-availability-status')
        ).toHaveAttribute('data-availability-status', 'pending_restart')
      );
      expect(screen.getByTestId('plugin-runtime-status')).toHaveAttribute(
        'data-runtime-status',
        'inactive'
      );
      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    }
  );

  test('warns before selecting a native version and keeps pending restart distinct from running', async () => {
    const installed = {
      id: 'installation-2',
      category: 'host-extensions',
      catalog_id: 'host-extensions:acme/preferences',
      organization: 'acme',
      artifact_id: 'preferences',
      version: '2.0.0',
      node_id: 'node-1',
      source_kind: 'local',
      trust_level: 'trusted_host',
      warnings: [],
      status: 'installed',
      is_current: true,
      desired_state: 'disabled',
      runtime_status: 'inactive',
      availability_status: 'disabled',
      application_action: 'none',
      application_status: 'not_required',
      installed_versions: [],
      created_by: 'user-1',
      created_at: '',
      updated_at: ''
    };
    api.listConsoleInstalledExtensions.mockResolvedValue({
      entries: [installed],
      total_entries: 1,
      limit: 20,
      next_cursor: null
    });
    api.enableConsoleInstalledExtension.mockImplementation(async () => {
      const pending = {
        ...installed,
        desired_state: 'active_requested',
        runtime_status: 'inactive',
        availability_status: 'pending_restart'
      };
      api.listConsoleInstalledExtensions.mockResolvedValue({
        entries: [pending],
        total_entries: 1,
        limit: 20,
        next_cursor: null
      });
      return { id: 'enable-task', status: 'completed' };
    });
    renderSurface(<SettingsExtensionCenterSection category="installed" />);
    const warning = await screen.findByTestId(
      'plugin-settings-upgrade-warning'
    );
    expect(warning.textContent).toContain('at restart');
    expect(warning.textContent).toContain('overwrite your edits');
    const control = await screen.findByRole('switch');
    fireEvent.click(control);
    await waitFor(() =>
      expect(api.enableConsoleInstalledExtension).toHaveBeenCalledWith(
        'installation-2',
        'csrf-123'
      )
    );
    await waitFor(() =>
      expect(screen.getByTestId('plugin-availability-status')).toHaveAttribute(
        'data-availability-status',
        'pending_restart'
      )
    );
    expect(screen.getByTestId('plugin-availability-status').textContent).toBe(
      'Pending restart'
    );
    expect(screen.getByTestId('plugin-runtime-status').textContent).toBe(
      'Inactive'
    );
  });
});
