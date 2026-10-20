import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, expect, type Mock, vi } from 'vitest';
import type { ConsoleMcpInterfaceCapability } from '@1flowbase/api-client';

const mcpManagementApi = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog'],
  settingsOfficialMcpBundlesQueryKey: [
    'settings',
    'mcp-management',
    'official-bundles'
  ],
  settingsMcpBundleExportDefaultsQueryKey: [
    'settings',
    'mcp-management',
    'bundle-export-defaults'
  ],
  settingsMcpTemplateLibraryQueryKey: [
    'settings',
    'mcp-management',
    'template-library'
  ],
  settingsMcpUpstreamConnectionsQueryKey: [
    'settings',
    'mcp-management',
    'upstream-connections'
  ],
  createSettingsMcpUpstreamConnection: vi.fn(),
  createSettingsMcpInstance: vi.fn(),
  copySettingsMcpInstance: vi.fn(),
  createSettingsMcpTool: vi.fn(),
  createSettingsMcpToolBinding: vi.fn(),
  deleteSettingsMcpClientCredential: vi.fn(),
  deleteSettingsMcpGroup: vi.fn(),
  deleteSettingsMcpInstance: vi.fn(),
  deleteSettingsMcpTool: vi.fn(),
  deleteSettingsMcpToolBinding: vi.fn(),
  deleteSettingsMcpTemplateLibraryRelease: vi.fn(),
  deleteSettingsMcpUpstreamConnection: vi.fn(),
  deleteSettingsMcpUpstreamConnectionCredentials: vi.fn(),
  discoverSettingsMcpUpstreamConnection: vi.fn(),
  executeSettingsMcpProxyToolDebug: vi.fn(),
  executeSettingsMcpToolDebug: vi.fn(),
  moveSettingsMcpGroup: vi.fn(),
  previewSettingsMcpBundle: vi.fn(),
  previewSettingsMcpTemplateLibraryBundle: vi.fn(),
  importSettingsMcpBundle: vi.fn(),
  importSettingsMcpTemplateLibraryBundle: vi.fn(),
  importSettingsOfficialMcpBundle: vi.fn(),
  exportSettingsMcpBundle: vi.fn(),
  exportSettingsMcpInstanceBundle: vi.fn(),
  exportSettingsMcpCatalog: vi.fn(),
  fetchSettingsMcpBundleExportDefaults: vi.fn(),
  fetchSettingsMcpTemplateLibrary: vi.fn(),
  fetchSettingsMcpClientCredential: vi.fn(
    async (): Promise<{ saved: boolean; api_key?: string }> => ({
      saved: false
    })
  ),
  fetchSettingsOfficialMcpBundles: vi.fn(),
  fetchSettingsMcpUpstreamConnections: vi.fn(async () => []),
  importSettingsMcpUpstreamTools: vi.fn(),
  previewSettingsOfficialMcpBundle: vi.fn(),
  refreshSettingsMcpToolDescription: vi.fn(),
  repairSettingsMcpTemplateLibraryRelease: vi.fn(),
  saveSettingsMcpClientCredential: vi.fn(async () => ({ saved: true })),
  saveSettingsMcpUpstreamConnectionCredentials: vi.fn(),
  setSettingsMcpTemplateLibraryCurrentVersion: vi.fn(),
  syncSettingsMcpTemplateLibraryBundle: vi.fn(),
  testSettingsMcpUpstreamConnection: vi.fn(),
  updateSettingsMcpInstance: vi.fn(),
  updateSettingsMcpInstanceDiscoveryPolicy: vi.fn(),
  updateSettingsMcpTool: vi.fn(),
  updateSettingsMcpToolBinding: vi.fn(),
  updateSettingsMcpUpstreamConnection: vi.fn(),
  upsertSettingsMcpGroup: vi.fn()
}));
const vditorMock = vi.hoisted(() => ({
  preview: vi.fn(async (target: HTMLDivElement, markdown: string) => {
    target.textContent = markdown;
  }),
  instances: [] as Array<{
    options: {
      mode?: string;
      value?: string;
      outline?: { enable?: boolean; position?: string };
      after?: () => void;
      input?: (value: string) => void;
    };
    setValue: Mock<(value: string, emitUpdate?: boolean) => void>;
    getValue: Mock<() => string>;
    destroy: Mock<() => void>;
  }>,
  constructor: vi.fn(function VditorMock(
    this: unknown,
    _target: HTMLElement,
    options: {
      mode?: string;
      value?: string;
      outline?: { enable?: boolean; position?: string };
      after?: () => void;
      input?: (value: string) => void;
    }
  ) {
    let currentValue = options.value ?? '';
    const instance = {
      options,
      setValue: vi.fn<(value: string, emitUpdate?: boolean) => void>(
        (value) => {
          currentValue = value;
        }
      ),
      getValue: vi.fn<() => string>(() => currentValue),
      destroy: vi.fn<() => void>()
    };
    vditorMock.instances.push(instance);

    return instance;
  })
}));

vi.mock('../../../../api/mcp-management', () => mcpManagementApi);
vi.mock('@tanstack/react-router', async () => {
  const React = await import('react');

  return {
    useRouterState: ({
      select
    }: {
      select: (state: {
        location: { search: Record<string, string> };
      }) => unknown;
    }) => {
      const search = React.useSyncExternalStore(
        (onStoreChange) => {
          window.addEventListener('popstate', onStoreChange);
          return () => window.removeEventListener('popstate', onStoreChange);
        },
        () => window.location.search,
        () => window.location.search
      );

      return select({
        location: {
          search: Object.fromEntries(new URLSearchParams(search))
        }
      });
    }
  };
});
vi.mock('vditor', () => {
  Object.assign(vditorMock.constructor, { preview: vditorMock.preview });
  return {
    __esModule: true,
    default: vditorMock.constructor
  };
});
vi.mock('vditor/dist/index.css', () => ({}));
vi.mock('@1flowbase/rich-text', async () => {
  const { createRichTextEditorFixture } =
    await import('./fixtures/rich-text-editor');
  return createRichTextEditorFixture(vditorMock);
});
vi.mock('@monaco-editor/react', () => ({
  __esModule: true,
  default: ({
    'aria-label': ariaLabel,
    options,
    value,
    onChange
  }: {
    'aria-label'?: string;
    options?: { ariaLabel?: string };
    value?: string;
    onChange?: (value?: string) => void;
  }) => (
    <textarea
      aria-label={ariaLabel ?? options?.ariaLabel}
      value={value ?? ''}
      onChange={(event) => onChange?.(event.target.value)}
    />
  )
}));

import { AppProviders } from '../../../../../../app/AppProviders';
import {
  resetAuthStore,
  useAuthStore
} from '../../../../../../state/auth-store';
import { McpManagementPanel } from '../../McpManagementPanel';

const interfaceCapabilities: ConsoleMcpInterfaceCapability[] = [
  {
    interface_id: 'create_app',
    method: 'POST',
    path: '/api/console/apps',
    name: 'Create app',
    short_description: 'Create app',
    parameter_schema: {
      type: 'object',
      properties: {
        app_id: {
          type: 'string',
          description: 'Application id'
        }
      },
      required: ['app_id']
    },
    parameter_descriptors: [
      {
        name: 'app_id',
        field_type: 'string',
        parameter_type: 'url' as const,
        description: 'Application id',
        required: true,
        schema: { type: 'string' }
      },
      {
        name: 'display_name',
        field_type: 'string',
        parameter_type: 'json_body' as const,
        description: 'Display name',
        required: false,
        schema: { type: 'string' }
      }
    ],
    result_schema: {
      type: 'object',
      properties: {
        run_id: {
          type: 'string',
          description: 'Flow run id'
        }
      }
    },
    permission_code: 'app.manage.all',
    security: {},
    risk_level: 'medium',
    bindable: true,
    disabled_reason: null
  }
];

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

async function renderPanel(
  capabilities: ConsoleMcpInterfaceCapability[] = interfaceCapabilities
) {
  return await renderSettled(
    <AppProviders>
      <McpManagementPanel
        canManage
        catalog={{
          instances: [],
          groups: [],
          tools: [],
          bindings: [],
          discovery_policies: []
        }}
        interfaceCapabilities={capabilities}
      />
    </AppProviders>
  );
}

async function renderPanelWithMountedTool({
  includeBinding = true,
  includeGroup = false,
  operation = 'POST /api/console/apps',
  proxy = false,
  fullDescription = 'Search customer'
}: {
  includeBinding?: boolean;
  includeGroup?: boolean;
  operation?: string;
  proxy?: boolean;
  fullDescription?: string;
} = {}) {
  return await renderSettled(
    <AppProviders>
      <McpManagementPanel
        canManage
        catalog={{
          instances: [
            {
              id: 'instance-record-1',
              workspace_id: 'workspace-1',
              instance_id: 'ops_mcp',
              name: 'Ops MCP',
              description_short: null,
              status: 'enabled',
              default_entry_path: '/',
              webmcp_exposure: 'disabled',
              managed_by: null,
              created_by: 'user-1',
              updated_by: 'user-1',
              created_at: '2026-07-06T00:00:00Z',
              updated_at: '2026-07-06T00:00:00Z',
              llm_tool_registration: {
                prefix: 'ops_mcp',
                tools: [
                  { operation: 'list', name: 'ops_mcp_mcp_list' },
                  { operation: 'get', name: 'ops_mcp_mcp_get' },
                  { operation: 'result', name: 'ops_mcp_mcp_result' },
                  { operation: 'call', name: 'ops_mcp_mcp_call' }
                ]
              }
            }
          ],
          groups: includeGroup
            ? [
                {
                  id: 'group-1',
                  instance_record_id: 'instance-record-1',
                  path: '/ops',
                  display_name: 'ops',
                  description_short: null,
                  enabled: true,
                  sort_order: 0
                }
              ]
            : [],
          tools: [
            {
              id: 'tool-record-1',
              workspace_id: 'workspace-1',
              tool_id: 'search_customer',
              name: 'Search customer',
              short_description: 'Find matching customers',
              full_description: fullDescription,
              managed_by: null,
              execution_target: proxy
                ? {
                    kind: 'mcp_proxy' as const,
                    upstream_connection_id: '019b-connection',
                    remote_tool_name: 'search_documents',
                    source_schema_hash: 'sha256:source'
                  }
                : {
                    kind: 'interface_wrapper' as const,
                    interface_id: 'create_app'
                  },
              operation,
              parameter_schema: {},
              result_schema: {},
              max_inline_chars: null,
              response_fields: null,
              input_mapping: proxy
                ? {
                    mappings: [
                      {
                        local_path: 'request.query',
                        remote_path: 'query.text',
                        required: true
                      }
                    ]
                  }
                : {},
              output_mapping: proxy
                ? {
                    mappings: [
                      {
                        remote_path: 'document.title',
                        local_path: 'result.title',
                        required: true
                      }
                    ]
                  }
                : {},
              permission_code: null,
              risk_level: 'low',
              des_id: 'des-1',
              des_id_required: false,
              status: 'enabled',
              availability_status: 'available',
              availability_reason: null,
              revision: 1
            }
          ],
          bindings: includeBinding
            ? [
                {
                  id: 'binding-1',
                  instance_record_id: 'instance-record-1',
                  tool_record_id: 'tool-record-1',
                  group_path: '/ops/customer',
                  tool_id: 'search_customer',
                  display_alias: null,
                  visible: true,
                  sort_order: 0
                }
              ]
            : [],
          discovery_policies: [
            {
              id: 'policy-1',
              workspace_id: 'workspace-1',
              instance_record_id: 'instance-record-1',
              instance_id: 'ops_mcp',
              list_default_limit: 20,
              list_max_depth: 3,
              list_regex_enabled: false,
              list_regex_max_length: 120,
              list_return_fields: []
            }
          ]
        }}
        interfaceCapabilities={interfaceCapabilities}
      />
    </AppProviders>
  );
}

async function selectAntdOption(label: string) {
  const [option] = await screen.findAllByText((_, element) => {
    return Boolean(
      element?.matches('.ant-select-item-option-content') &&
      element.textContent?.includes(label)
    );
  });

  await domEvents.click(option);
}

async function clickSegmentedOption(root: HTMLElement, label: string) {
  const option = within(root).getByText((text, element) => {
    return Boolean(
      text === label && element?.matches('.ant-segmented-item-label')
    );
  });

  await domEvents.click(option);
}

function visibleTextEntries(root: HTMLElement, text: string) {
  return within(root)
    .getAllByText(text)
    .filter((entry) => !entry.closest('[hidden]'));
}

async function setFullDescription(value: string) {
  await waitFor(() => {
    expect(vditorMock.instances.at(-1)).toMatchObject({
      options: { input: expect.any(Function) }
    });
  });
  const editor = vditorMock.instances.at(-1);

  await act(async () => {
    editor!.options.input?.(value);
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  resetAuthStore();
  authenticate();
  vditorMock.instances.length = 0;
  window.history.replaceState({}, '', '/settings/mcp-management');
  mcpManagementApi.fetchSettingsMcpClientCredential.mockResolvedValue({
    saved: false
  });
  mcpManagementApi.fetchSettingsMcpBundleExportDefaults.mockResolvedValue({
    current_system_version: '0.3.0'
  });
  mcpManagementApi.saveSettingsMcpClientCredential.mockResolvedValue({
    saved: true
  });
  mcpManagementApi.deleteSettingsMcpClientCredential.mockResolvedValue(
    undefined
  );
  mcpManagementApi.executeSettingsMcpToolDebug.mockImplementation(
    async (body: { debug_response_mode?: string; mcp_arguments: unknown }) =>
      body.debug_response_mode === 'debug_details'
        ? {
            mcp_arguments: body.mcp_arguments,
            interface_arguments: {
              body: body.mcp_arguments
            },
            interface_response: {
              data: body.mcp_arguments
            },
            tool_result: body.mcp_arguments
          }
        : body.mcp_arguments
  );
  mcpManagementApi.previewSettingsMcpBundle.mockResolvedValue({
    manifest: {
      schema_version: '1flowbase.mcp.bundle/v2',
      organization: 'taichuy',
      bundle_id: '1flowbase_zh_hans',
      bundle_version: '1.0.0',
      locale: 'zh_Hans',
      minimum_host_version: '0.2.6',
      exported_from_system_version: '0.2.5',
      exported_at: '2026-07-13T10:00:00Z',
      files: [
        {
          path: 'connections/019b5f8f.json',
          kind: 'connection',
          sha256: 'connection-sha256'
        }
      ]
    },
    current_system_version: '0.2.6',
    version_status: 'exported_from_older_system',
    effect_summary: {
      changes: 4,
      already_present: 0,
      conflicts: 0,
      unavailable: 2,
      failed: 0
    },
    tools: [
      {
        id: 'runtime_profile',
        effect: 'create',
        result: 'imported',
        reason: null
      },
      {
        id: 'removed_tool',
        effect: 'create',
        result: 'unavailable',
        reason: 'interface_missing'
      }
    ],
    instances: [
      {
        id: 'system',
        effect: 'create',
        result: 'imported',
        reason: null
      }
    ],
    connections: [
      {
        id: '019b5f8f-0000-7000-8000-000000000001',
        effect: 'create',
        result: 'unavailable',
        reason: 'credentials_missing'
      }
    ],
    shared_tool_impacts: []
  });
  mcpManagementApi.importSettingsMcpBundle.mockResolvedValue({
    manifest: {
      schema_version: '1flowbase.mcp.bundle/v2',
      organization: 'taichuy',
      bundle_id: '1flowbase_zh_hans',
      bundle_version: '1.0.0',
      locale: 'zh_Hans',
      minimum_host_version: '0.2.6',
      exported_from_system_version: '0.2.5',
      exported_at: '2026-07-13T10:00:00Z',
      files: [
        {
          path: 'connections/019b5f8f.json',
          kind: 'connection',
          sha256: 'connection-sha256'
        }
      ]
    },
    current_system_version: '0.2.6',
    version_status: 'exported_from_older_system',
    status: 'completed_with_warnings',
    effect_summary: {
      changes: 4,
      already_present: 0,
      conflicts: 0,
      unavailable: 2,
      failed: 0
    },
    tools: [
      {
        id: 'runtime_profile',
        effect: 'create',
        result: 'imported',
        reason: null
      },
      {
        id: 'removed_tool',
        effect: 'create',
        result: 'unavailable',
        reason: 'interface_missing'
      }
    ],
    instances: [
      {
        id: 'system',
        effect: 'create',
        result: 'imported',
        reason: null
      }
    ],
    connections: [
      {
        id: '019b5f8f-0000-7000-8000-000000000001',
        effect: 'create',
        result: 'unavailable',
        reason: 'credentials_missing'
      }
    ],
    shared_tool_impacts: []
  });
  mcpManagementApi.fetchSettingsMcpTemplateLibrary.mockResolvedValue({
    remote_available: true,
    bundles: [
      {
        organization: 'taichuy',
        bundle_id: '1flowbase_zh_hans',
        current_bundle_version: '1.0.0',
        remote_versions: [],
        local_versions: [
          {
            bundle_version: '1.0.0',
            locale: 'zh_Hans',
            minimum_host_version: '0.2.6',
            exported_from_system_version: '0.2.5',
            checksum: 'bundle-sha256',
            signature_status: 'verified',
            downloaded_at: '2026-08-02T10:00:00Z'
          }
        ]
      }
    ]
  });
  mcpManagementApi.previewSettingsMcpTemplateLibraryBundle.mockImplementation(
    async () => mcpManagementApi.previewSettingsMcpBundle()
  );
  mcpManagementApi.importSettingsMcpTemplateLibraryBundle.mockImplementation(
    async () => mcpManagementApi.importSettingsMcpBundle()
  );
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => {
    result = event();
  });
  return result;
}

// Explicit render owners for this fixture; other test roots remain RTL-owned.
const mountedViews = new Set<ReturnType<typeof render>>();

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(...args);
    mountedViews.add(view);
  });
  return view;
}

const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) =>
    dispatch(() => fireEvent.click(...args)),
  change: (...args: Parameters<typeof fireEvent.change>) =>
    dispatch(() => fireEvent.change(...args)),
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) =>
    dispatch(() => fireEvent.mouseDown(...args))
};

afterEach(() => {
  // Release only the render owners created by this fixture before its resource teardown.
  for (const view of mountedViews) view.unmount();
  mountedViews.clear();
  for (const editor of vditorMock.instances)
    expect(editor.destroy).toHaveBeenCalledOnce();
});

export {
  clickSegmentedOption,
  domEvents,
  mcpManagementApi,
  renderPanel,
  renderPanelWithMountedTool,
  renderSettled,
  selectAntdOption,
  setFullDescription,
  vditorMock,
  visibleTextEntries
};
