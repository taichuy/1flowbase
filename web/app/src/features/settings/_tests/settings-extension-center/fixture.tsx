import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  cleanup,
  fireEvent,
  render,
  waitFor
} from '@testing-library/react';
import { App, Modal } from 'antd';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, expect, vi } from 'vitest';

const extensionsApi = vi.hoisted(() => ({
  settingsInstalledExtensionsQueryKey: vi.fn(
    (cursor?: string, category?: string) => [
      'settings',
      'extension-center',
      'installed',
      category ?? 'all',
      cursor ?? 'start'
    ]
  ),
  settingsExtensionCatalogQueryKey: vi.fn(
    (
      category: string,
      query: { q?: string; slot_code?: string; cursor?: string }
    ) => [
      'settings',
      'extension-center',
      'catalog',
      category,
      query.q ?? '',
      query.slot_code ?? 'all-slots',
      query.cursor ?? 'start'
    ]
  ),
  fetchSettingsInstalledExtensions: vi.fn(),
  fetchSettingsExtensionCatalog: vi.fn(),
  fetchSettingsExtensionCatalogEntry: vi.fn(),
  checkSettingsExtensionUpdates: vi.fn(),
  installSettingsExtension: vi.fn(),
  switchSettingsPluginFamilyVersion: vi.fn(),
  uploadSettingsExtension: vi.fn(),
  enableSettingsInstalledExtension: vi.fn(),
  disableSettingsInstalledExtension: vi.fn(),
  deleteSettingsInstalledExtension: vi.fn(),
  getSettingsExtensionRiskChallenge: vi.fn(),
  previewSettingsInstalledMcpExtension: vi.fn(),
  applySettingsInstalledMcpExtension: vi.fn(),
  getSettingsInstalledMcpExtensionConflict: vi.fn(),
  getSettingsInstalledMcpExtensionIntegrityChallenge: vi.fn()
}));

const i18nCatalogApi = vi.hoisted(() => ({
  settingsI18nCatalogQueryKey: ['settings', 'i18n-catalog'] as const,
  previewSettingsInstalledI18nCatalog: vi.fn(),
  activateSettingsInstalledI18nCatalog: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));

const mcpManagementApi = vi.hoisted(() => ({
  settingsMcpInterfaceCapabilitiesQueryKey: [
    'settings',
    'mcp-management',
    'interface-capabilities'
  ] as const,
  settingsMcpCatalogQueryKey: [
    'settings',
    'mcp-management',
    'catalog'
  ] as const,
  previewSettingsMcpBundle: vi.fn(),
  importSettingsMcpBundle: vi.fn(),
  previewSettingsOfficialMcpBundle: vi.fn(),
  importSettingsOfficialMcpBundle: vi.fn()
}));

const routerApi = vi.hoisted(() => ({ navigate: vi.fn() }));

vi.mock('../../api/extensions/extensions', () => extensionsApi);
vi.mock('../../api/extensions/plugins', () => ({
  switchSettingsPluginFamilyVersion:
    extensionsApi.switchSettingsPluginFamilyVersion
}));
vi.mock('../../api/i18n-catalog', () => i18nCatalogApi);
vi.mock('../../../applications/api/applications', () => applicationsApi);
vi.mock('../../api/mcp-management', () => mcpManagementApi);
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => routerApi.navigate
}));

import { AppI18nProvider } from '../../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { SettingsExtensionCenterSection } from '../../pages/settings-page/SettingsExtensionCenterSection';

const installedEntry = {
  contract_version: null,
  id: 'extension-installation-1',
  category: 'runtime-extensions' as const,
  catalog_id: 'runtime-extensions:taichuy/openai',
  organization: 'taichuy',
  artifact_id: 'openai',
  version: '1.0.0',
  node_id: 'node-1',
  source_kind: 'official',
  trust_level: 'official',
  warnings: [],
  local_path: '/api/plugins/openai',
  expected_checksum: 'sha256:installed',
  local_checksum: 'sha256:installed',
  signature_status: 'valid',
  signature_algorithm: 'ed25519',
  signing_key_id: 'official-key',
  status: 'installed',
  is_current: true,
  desired_state: 'active_requested',
  availability_status: 'available',
  application_action: 'configure_model_provider' as const,
  application_status: 'available' as const,
  created_by: 'user-1',
  created_at: '2026-08-01T10:00:00Z',
  updated_at: '2026-08-01T10:00:00Z',
  installed_versions: [
    {
      id: 'extension-installation-1',
      version: '1.0.0',
      source_kind: 'official',
      trust_level: 'official',
      warnings: [],
      local_path: '/api/plugins/openai/1.0.0',
      expected_checksum: 'sha256:installed',
      local_checksum: 'sha256:installed',
      signature_status: 'valid',
      signature_algorithm: 'ed25519',
      signing_key_id: 'official-key',
      status: 'installed',
      is_current: true,
      deletable: false,
      delete_reasons: ['current_version'],
      created_by: 'user-1',
      created_at: '2026-08-01T10:00:00Z',
      updated_at: '2026-08-01T10:00:00Z'
    },
    {
      id: 'extension-installation-0',
      version: '0.9.0',
      source_kind: 'upload',
      trust_level: 'unknown',
      warnings: [],
      local_path: '/api/plugins/openai/0.9.0',
      expected_checksum: 'sha256:previous',
      local_checksum: 'sha256:previous',
      signature_status: 'missing',
      signature_algorithm: null,
      signing_key_id: null,
      status: 'installed',
      is_current: false,
      deletable: true,
      delete_reasons: [],
      created_by: 'user-1',
      created_at: '2026-07-01T10:00:00Z',
      updated_at: '2026-07-01T10:00:00Z'
    }
  ]
};

const catalogEntry = {
  category: 'runtime-extensions' as const,
  id: 'runtime-extensions:taichuy/openai',
  name: 'OpenAI Provider',
  organization: 'taichuy',
  artifact: 'openai',
  version: '1.1.0',
  description: 'Remote provider extension',
  host_version_requirement: '>=0.4.0',
  source: { kind: 'github_release' },
  signature: { key_id: 'official-key' },
  checksum: 'sha256:catalog-entry',
  download_locator: { url: 'https://example.com/openai.1flowbasepkg' },
  catalog_page: 1,
  catalog_source: 'official',
  current_version: '1.0.0',
  installation_status: 'installed',
  artifact_kind: 'model_provider',
  installation_source: 'official',
  extension_installation_id: null,
  builtin_template_id: null,
  trust: 'official',
  warnings: [],
  compatibility: null,
  slot_codes: ['model_provider'],
  keywords: ['openai', 'provider'],
  mcp_instances: []
};

const installableCatalogEntry = {
  ...catalogEntry,
  current_version: null,
  installation_status: 'not_installed' as const
};

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'root',
      name: 'root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['system.extension-center.manage.all']
    }
  });
}

async function renderSection(
  category:
    | 'installed'
    | 'agent-flow'
    | 'capability-plugins'
    | 'host-extensions'
    | 'i18n'
    | 'mcp'
    | 'runtime-extensions' = 'installed',
  cursor?: string,
  q?: string
) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
  mountedClients.add(client);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <App>
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </App>
    </AppI18nProvider>
  );

  return {
    ...(await renderSettled(
      <SettingsExtensionCenterSection
        category={category}
        cursor={cursor}
        q={q}
      />,
      { wrapper }
    )),
    queryClient: client
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return {
    promise,
    resolve: async (value: T) => {
      await act(async () => {
        resolve(value);
      });
    }
  };
}


async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(...args);
  });
  return view;
}
async function click(...args: Parameters<typeof fireEvent.click>) {
  await act(async () => {
    fireEvent.click(...args);
  });
}
async function change(...args: Parameters<typeof fireEvent.change>) {
  await act(async () => {
    fireEvent.change(...args);
  });
}
async function keyDown(...args: Parameters<typeof fireEvent.keyDown>) {
  await act(async () => {
    fireEvent.keyDown(...args);
  });
}

const mountedClients = new Set<QueryClient>();

export function setupExtensionCenterFixture() {
  beforeEach(() => {
    vi.clearAllMocks();
    // Clear one-shot response queues as well as call history between scenarios.
    extensionsApi.fetchSettingsInstalledExtensions.mockReset();
    extensionsApi.fetchSettingsExtensionCatalog.mockReset();
    resetAuthStore();
    authenticate();
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [installedEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [catalogEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalogEntry.mockResolvedValue(
      catalogEntry
    );
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'runtime-extensions',
      items: [
        {
          catalog_id: 'runtime-extensions:taichuy/openai',
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });
    extensionsApi.installSettingsExtension.mockResolvedValue({
      installation: installedEntry,
      local_artifact_was_present: false,
      node_plugin_installation_id: 'plugin-installation-1',
      application_action: 'configure_model_provider',
      application_status: 'available'
    });
    extensionsApi.deleteSettingsInstalledExtension.mockResolvedValue(undefined);
    extensionsApi.enableSettingsInstalledExtension.mockResolvedValue({
      ...installedEntry,
      desired_state: 'active_requested',
      availability_status: 'available'
    });
    extensionsApi.disableSettingsInstalledExtension.mockResolvedValue({
      ...installedEntry,
      desired_state: 'disabled',
      availability_status: 'disabled'
    });
    extensionsApi.getSettingsExtensionRiskChallenge.mockReturnValue(null);
    extensionsApi.getSettingsInstalledMcpExtensionConflict.mockReturnValue(
      null
    );
    extensionsApi.getSettingsInstalledMcpExtensionIntegrityChallenge.mockReturnValue(
      null
    );
    applicationsApi.previewInstalledApplicationExtension.mockResolvedValue({
      extension_installation_id: 'agent-installation-1',
      application_status: 'not_applied',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        application: {
          application_type: 'agent_flow',
          name: 'Imported flow',
          description: 'Flow description',
          icon: null,
          icon_type: null,
          icon_background: null
        },
        dependencies: [],
        unresolved_nodes: [],
        flow_document: {}
      }
    });
    vi.spyOn(Modal, 'confirm').mockReturnValue({ destroy: vi.fn() } as never);
  });

  afterEach(async () => {
    try {
      // Mutation onError returns Antd's message thenable, which settles after
      // the real toast closes; its default lifetime exceeds waitFor's 1 s.
      await waitFor(() => {
        for (const client of mountedClients) {
          expect(client.isFetching()).toBe(0);
          expect(client.isMutating()).toBe(0);
        }
      }, { timeout: 5000 });
    } finally {
      cleanup();
      for (const client of mountedClients) client.clear();
      mountedClients.clear();
      await act(async () => {
        resetAuthStore();
      });
    }
  });
}

export {
  extensionsApi,
  i18nCatalogApi,
  applicationsApi,
  mcpManagementApi,
  routerApi,
  installedEntry,
  catalogEntry,
  installableCatalogEntry,
  renderSection,
  deferred,
  renderSettled,
  click,
  change,
  keyDown
};
