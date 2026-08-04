import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
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
      after?: () => void;
      input?: (value: string) => void;
    };
    setValue: ReturnType<typeof vi.fn>;
    getValue: ReturnType<typeof vi.fn>;
    destroy: ReturnType<typeof vi.fn>;
  }>,
  constructor: vi.fn(function VditorMock(
    this: unknown,
    _target: HTMLElement,
    options: {
      mode?: string;
      value?: string;
      after?: () => void;
      input?: (value: string) => void;
    }
  ) {
    let currentValue = options.value ?? '';
    const instance = {
      options,
      setValue: vi.fn((value: string) => {
        currentValue = value;
      }),
      getValue: vi.fn(() => currentValue),
      destroy: vi.fn()
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
import { McpToolDebugPanel } from '../../McpToolDebugPanel';
import { MarkdownIrEditor } from '../../../../../../shared/ui/markdown-ir-editor/MarkdownIrEditor';

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

const publishApplicationApiCapability: ConsoleMcpInterfaceCapability = {
  ...interfaceCapabilities[0],
  interface_id: 'publish_application_api',
  method: 'POST',
  path: '/api/console/applications/{application_id}/api-publications',
  name: 'Publish application API',
  short_description: 'Publish application API',
  parameter_schema: {
    type: 'object',
    properties: {
      application_id: { type: 'string' },
      api_enabled: { type: 'boolean' },
      mapping: {
        type: 'object',
        properties: {
          input: {
            type: 'object',
            properties: {
              query_target: { type: 'string' },
              history_target: { type: 'string' }
            },
            required: ['query_target']
          },
          output: {
            type: 'object',
            properties: {
              answer_selector: { type: 'string' }
            }
          }
        },
        required: ['input', 'output']
      }
    },
    required: ['application_id', 'api_enabled', 'mapping']
  },
  parameter_descriptors: [
    {
      name: 'application_id',
      field_type: 'string',
      parameter_type: 'url' as const,
      description: 'Application id',
      required: true,
      schema: { type: 'string' }
    },
    {
      name: 'api_enabled',
      field_type: 'boolean',
      parameter_type: 'json_body' as const,
      description: 'API enabled',
      required: true,
      schema: { type: 'boolean' }
    },
    {
      name: 'mapping.input.query_target',
      field_type: 'string',
      parameter_type: 'json_body' as const,
      description: 'Query target',
      required: true,
      schema: { type: 'string' }
    },
    {
      name: 'mapping.input.history_target',
      field_type: 'string',
      parameter_type: 'json_body' as const,
      description: 'History target',
      required: false,
      schema: { type: 'string' }
    },
    {
      name: 'mapping.output.answer_selector',
      field_type: 'string',
      parameter_type: 'json_body' as const,
      description: 'Answer selector',
      required: false,
      schema: { type: 'string' }
    }
  ]
};

function renderPanel(
  capabilities: ConsoleMcpInterfaceCapability[] = interfaceCapabilities
) {
  return render(
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

function renderPanelWithMountedTool({
  includeBinding = true,
  includeGroup = false,
  operation = 'POST /api/console/apps',
  proxy = false
}: {
  includeBinding?: boolean;
  includeGroup?: boolean;
  operation?: string;
  proxy?: boolean;
} = {}) {
  return render(
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
              created_by: 'user-1',
              updated_by: 'user-1',
              created_at: '2026-07-06T00:00:00Z',
              updated_at: '2026-07-06T00:00:00Z'
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
              full_description: 'Search customer',
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

  fireEvent.click(option);
}

function clickSegmentedOption(root: HTMLElement, label: string) {
  const option = within(root).getByText((text, element) => {
    return Boolean(
      text === label && element?.matches('.ant-segmented-item-label')
    );
  });

  fireEvent.click(option);
}

function expandTreeRootIfCollapsed(tree: HTMLElement) {
  const rootItem = within(tree).getAllByRole('treeitem')[0];
  if (rootItem?.getAttribute('aria-expanded') === 'false') {
    const switcher = rootItem.querySelector('.ant-tree-switcher');
    expect(switcher).toBeInstanceOf(HTMLElement);
    fireEvent.click(switcher as HTMLElement);
  }
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

  act(() => {
    editor!.options.input?.(value);
  });
}

describe('McpManagementPanel', () => {
  afterEach(async () => {
    cleanup();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });

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

  test('confirms before closing an editor with unsaved changes', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Unsaved group' }
    });
    await waitFor(() => {
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Unsaved group'
      );
    });
    fireEvent.click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );

    expect(screen.getByText('放弃未保存的更改？')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '放弃更改' })
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '继续编辑' }));
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
  });

  test('shows explicit group and Tool creation actions without editor tabs', () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));
    expect(
      dialog.querySelector('.mcp-management__directory-editor-status')
    ).toHaveTextContent('未保存');

    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('tab', { name: '分组' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole('tab', { name: 'Tool 挂载' })
    ).not.toBeInTheDocument();
  });

  test('does not preview a duplicate draft group when the path already exists', () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);

    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'ops' }
    });

    expect(within(tree).getAllByText('ops')).toHaveLength(1);
  });

  test('hides directory sort order fields because ordering is handled by tree drag', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    expect(
      within(dialog).queryByLabelText('sort_order')
    ).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(
      within(dialog).queryByLabelText('sort_order')
    ).not.toBeInTheDocument();
  });

  test('places directory cancel and save actions in the modal footer for both editor tabs', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const modalScrollBody = screen.getByTestId(
      'fixed-height-modal-scroll-body'
    );

    const cancelButton = within(dialog).getByRole('button', {
      name: '关闭目录编辑'
    });
    const saveButton = within(dialog).getByRole('button', { name: /保存/ });

    expect(cancelButton).toBeInTheDocument();
    expect(saveButton).toBeInTheDocument();
    expect(
      cancelButton.compareDocumentPosition(saveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(modalScrollBody).queryByRole('button', { name: /取\s*消/ })
    ).not.toBeInTheDocument();
    expect(
      within(modalScrollBody).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    const bindingCancelButton = within(dialog).getByRole('button', {
      name: '关闭目录编辑'
    });
    const bindingSaveButton = within(dialog).getByRole('button', {
      name: /保存/
    });

    expect(bindingCancelButton).toBeInTheDocument();
    expect(bindingSaveButton).toBeInTheDocument();
    expect(
      bindingCancelButton.compareDocumentPosition(bindingSaveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(modalScrollBody).queryByRole('button', { name: /取\s*消/ })
    ).not.toBeInTheDocument();
    expect(
      within(modalScrollBody).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();
  });

  test('renders draft group nodes with the display name and short description in the tree', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    const rootLabel = within(dialog).getByText('Ops MCP /');
    const rootNode = rootLabel.closest('.ant-tree-node-content-wrapper');
    expect(rootNode).toBeInstanceOf(HTMLElement);
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));
    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Customer Ops' }
    });
    fireEvent.change(within(dialog).getByLabelText('简短描述'), {
      target: { value: 'Tools for customer operations' }
    });

    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent(
        'Customer Ops'
      );
    });
    const currentTree = within(dialog).getByRole('tree');
    expect(currentTree).toHaveTextContent('Tools for customer operations');
    expect(currentTree).not.toHaveTextContent('Customer Ops /customer_ops');
  });

  test('hides the directory tree drag handle while keeping nodes draggable', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const tree = within(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).getByRole('tree');

    expect(
      tree.querySelector('.ant-tree-draggable-icon')
    ).not.toBeInTheDocument();
  });

  test('renders draft binding nodes with the tool id and short description in the tree', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(dialog).getByText('Ops MCP /');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'tool_id' })
    );
    await selectAntdOption('Search customer');

    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent(
        'search_customer'
      );
    });
    const currentTree = within(dialog).getByRole('tree');
    expect(currentTree).toHaveTextContent('Find matching customers');
    expect(currentTree).not.toHaveTextContent(
      'Search customer search_customer'
    );
  });

  test('replaces an unsaved Tool draft when starting a group creation session', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(dialog).getByText('Ops MCP /');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent('未命名 Tool');
    });

    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));

    expect(within(dialog).getByRole('tree')).not.toHaveTextContent(
      '未命名 Tool'
    );
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue('');
  });

  test('moves the single Tool draft when adding under another group', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    const rootLabel = within(dialog).getByText('Ops MCP /');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent('未命名 Tool');
    });

    const groupLabel = within(dialog).getByText('ops');
    fireEvent.click(groupLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    await waitFor(() => {
      expect(
        within(within(dialog).getByRole('tree')).getAllByText('未命名 Tool')
      ).toHaveLength(1);
      expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/ops');
    });
  });

  test('does not carry a draft group path into the binding mount path', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.change(within(dialog).getByLabelText('路径'), {
      target: { value: '/ops/customer' }
    });

    expect(within(dialog).getByLabelText('路径')).toHaveValue('/ops/customer');

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/');
  });

  test('falls back to interface id when a stale tool response misses operation', () => {
    renderPanelWithMountedTool({ operation: '' });

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(
      within(toolsPanel).getAllByText('create_app').length
    ).toBeGreaterThan(0);
  });

  test('generates a random instance_id from the instance modal action', async () => {
    const randomSpy = vi.spyOn(Math, 'random').mockReturnValue(0.123456789);

    try {
      renderPanel();

      fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
      fireEvent.click(screen.getByRole('button', { name: /新增/ }));

      const dialog = await screen.findByRole('dialog');
      const instanceIdField = within(dialog).getByLabelText('instance_id');

      expect(instanceIdField).toHaveValue('');

      fireEvent.click(
        within(dialog).getByRole('button', { name: '随机生成 instance_id' })
      );

      expect(instanceIdField).toHaveValue('04fzzzxj');
    } finally {
      randomSpy.mockRestore();
    }
  });

  test('shows step navigation actions only when the adjacent step exists', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).queryByRole('button', { name: /上一步/ })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: /下一步/ })
    ).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: /下一步/ }));
    expect(
      within(dialog).getByRole('button', { name: /上一步/ })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: /下一步/ })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('operation')).toBeInTheDocument();

    clickSegmentedOption(dialog, 'debug');
    expect(
      within(dialog).getByRole('button', { name: /上一步/ })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: /下一步/ })
    ).not.toBeInTheDocument();
  }, 30000);

  test('places the status selector before description fields in the basic step', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');
    const desIdField = within(dialog).getByLabelText('des_id');
    const statusField = within(dialog).getByRole('combobox', {
      name: 'status'
    });
    const shortDescriptionField =
      within(dialog).getByLabelText('short_description');
    const fullDescriptionField =
      within(dialog).getByLabelText('full_description');

    expect(
      desIdField.compareDocumentPosition(statusField) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      statusField.compareDocumentPosition(shortDescriptionField) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      statusField.compareDocumentPosition(fullDescriptionField) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  test('opens the requested tab from the URL search param', () => {
    window.history.replaceState({}, '', '/settings/mcp-management?tab=tools');

    renderPanel();

    expect(screen.getByRole('tab', { name: 'Tool 配置' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });

  test('AC-001 restores the third-party MCP tab from the URL', () => {
    window.history.replaceState(
      {},
      '',
      '/settings/mcp-management?tab=third-party'
    );

    renderPanel();

    expect(screen.getAllByRole('tab')).toHaveLength(3);
    expect(screen.getByRole('tab', { name: '第三方MCP' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });

  test('AC-002 AC-009 AC-010 AC-011 AC-014 edits the local MCP proxy contract without HTTP mapping fields', async () => {
    mcpManagementApi.executeSettingsMcpProxyToolDebug.mockResolvedValue({
      local_arguments: { request: { query: 'status' } },
      remote_arguments: { query: { text: 'status' } },
      upstream_result: { structuredContent: { document: { title: 'Status' } } },
      mapped_result: { structuredContent: { result: { title: 'Status' } } }
    });
    renderPanelWithMountedTool({ includeBinding: false, proxy: true });

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    expect(
      screen.getByRole('columnheader', { name: 'Tool 类型' })
    ).toBeInTheDocument();
    const typeFilter = screen.getByRole('combobox', { name: 'Tool 类型' });
    fireEvent.mouseDown(typeFilter);
    await selectAntdOption('接口封装');
    expect(
      screen.queryByRole('row', { name: /Search customer/ })
    ).not.toBeInTheDocument();
    fireEvent.mouseDown(typeFilter);
    await selectAntdOption('MCP 代理');
    const toolRow = screen.getByRole('row', { name: /Search customer/ });
    expect(within(toolRow).getByText('MCP 代理')).toBeInTheDocument();
    fireEvent.click(within(toolRow).getAllByRole('button')[0]);

    const dialog = await screen.findByRole('dialog');
    clickSegmentedOption(dialog, 'interface');
    expect(within(dialog).getByText('019b-connection')).toBeInTheDocument();
    expect(within(dialog).getByText('search_documents')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('combobox', { name: 'operation' })
    ).not.toBeInTheDocument();
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'risk_level' })
    );
    await selectAntdOption('medium');

    clickSegmentedOption(dialog, 'input_mapping');
    expect(within(dialog).getByText('parameter_schema')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(
      await within(dialog).findByLabelText('JSON Schema 内容', undefined, {
        timeout: 10000
      }),
      {
        target: {
          value: JSON.stringify({
            type: 'object',
            properties: { local_query: { type: 'string' } },
            required: ['local_query']
          })
        }
      }
    );
    expect(within(dialog).getByLabelText('local_path 1')).toHaveValue(
      'request.query'
    );
    expect(within(dialog).getByLabelText('remote_path 1')).toHaveValue(
      'query.text'
    );
    expect(within(dialog).queryByText('URL')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('JSON 请求体')).not.toBeInTheDocument();

    clickSegmentedOption(dialog, 'output_mapping');
    expect(within(dialog).getByText('result_schema')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(await within(dialog).findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify({
          type: 'object',
          properties: { local_title: { type: 'string' } }
        })
      }
    });

    clickSegmentedOption(dialog, 'debug');
    fireEvent.change(within(dialog).getByLabelText('request.query'), {
      target: { value: 'status' }
    });
    fireEvent.click(
      within(dialog).getByRole('button', { name: '运行代理调试' })
    );

    await waitFor(() => {
      expect(
        mcpManagementApi.executeSettingsMcpProxyToolDebug
      ).toHaveBeenCalledWith(
        'search_customer',
        { arguments: { request: { query: 'status' } } },
        expect.any(String)
      );
    });
    expect(
      await within(dialog).findByText('远端 arguments')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('第三方原始结果')).toBeInTheDocument();
    expect(within(dialog).getByText('本地映射结果')).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    await waitFor(() => {
      expect(mcpManagementApi.updateSettingsMcpTool).toHaveBeenCalledWith(
        'search_customer',
        expect.objectContaining({
          execution_target: {
            kind: 'mcp_proxy',
            upstream_connection_id: '019b-connection',
            remote_tool_name: 'search_documents',
            source_schema_hash: 'sha256:source'
          },
          input_mapping: {
            mappings: [
              {
                local_path: 'request.query',
                remote_path: 'query.text',
                required: true
              }
            ]
          },
          parameter_schema: {
            type: 'object',
            properties: { local_query: { type: 'string' } },
            required: ['local_query']
          },
          result_schema: {
            type: 'object',
            properties: { local_title: { type: 'string' } }
          },
          risk_level: 'medium'
        }),
        expect.any(String)
      );
    });
  }, 30_000);

  test('falls back from the removed meta tab to instances', async () => {
    window.history.replaceState({}, '', '/settings/mcp-management?tab=meta');
    renderPanel();

    await waitFor(() => {
      expect(window.location.search).toBe('?tab=instances');
    });
    expect(
      screen.queryByRole('tab', { name: 'MCP 配置' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'MCP 实例' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });

  test('edits only the selected instance discovery policy', async () => {
    mcpManagementApi.updateSettingsMcpInstanceDiscoveryPolicy.mockResolvedValue(
      {}
    );
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    fireEvent.click(
      within(menu).getByRole('menuitem', { name: /目录发现配置$/ })
    );

    const dialog = screen.getByRole('dialog', {
      name: '目录发现配置 · Ops MCP'
    });
    expect(
      within(dialog).getByTestId('fixed-height-modal-scroll-body')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('ops_mcp')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('默认返回数量')).toHaveValue('20');
    expect(within(dialog).getByLabelText('最大目录深度')).toHaveValue('3');
    expect(within(dialog).getByLabelText('允许正则路径查询')).not.toBeChecked();
    expect(within(dialog).getByLabelText('正则表达式最大长度')).toHaveValue(
      '120'
    );
    expect(
      within(dialog).getByRole('tab', { name: 'Schema 字段' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('tab', { name: 'JSON 解析' })
    ).toBeInTheDocument();
    fireEvent.click(
      within(dialog).getByRole('button', { name: /添加返回字段/ })
    );
    fireEvent.change(within(dialog).getByLabelText('列表返回字段 1'), {
      target: { value: 'id' }
    });
    expect(
      within(dialog).queryByText('包含参数映射摘要')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('描述版本校验')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('参数错误格式')).not.toBeInTheDocument();

    fireEvent.change(within(dialog).getByLabelText('默认返回数量'), {
      target: { value: '30' }
    });
    fireEvent.change(within(dialog).getByLabelText('最大目录深度'), {
      target: { value: '4' }
    });
    fireEvent.click(within(dialog).getByLabelText('允许正则路径查询'));
    fireEvent.change(within(dialog).getByLabelText('正则表达式最大长度'), {
      target: { value: '160' }
    });
    fireEvent.click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(
      await within(dialog).findByLabelText('列表返回字段 JSON'),
      {
        target: { value: '["id","name"]' }
      }
    );
    fireEvent.click(within(dialog).getByRole('button', { name: /保存/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.updateSettingsMcpInstanceDiscoveryPolicy
      ).toHaveBeenCalledWith(
        'ops_mcp',
        {
          list_default_limit: 30,
          list_max_depth: 4,
          list_regex_enabled: true,
          list_regex_max_length: 160,
          list_return_fields: ['id', 'name']
        },
        expect.any(String)
      );
    });
  });

  test('AC-001 AC-002 fills a temporary API key into the common MCP JSON tab', async () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));

    expect(await screen.findByText('MCP 客户端配置')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toHaveTextContent(
      `${window.location.origin}/api/mcp/ops_mcp`
    );
    fireEvent.change(screen.getByLabelText('API Key'), {
      target: { value: 'test-secret-key' }
    });

    const configuration = JSON.stringify(
      {
        type: 'http',
        url: `${window.location.origin}/api/mcp/ops_mcp`,
        headers: { Authorization: 'Bearer test-secret-key' }
      },
      null,
      2
    );
    expect(
      JSON.parse(screen.getByLabelText('完整 JSON 配置 JSON').textContent ?? '')
    ).toEqual(JSON.parse(configuration));
    expect(
      screen.getByRole('button', { name: '复制完整 JSON 配置' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '放大查看完整 JSON 配置' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '完整 JSON 配置' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('生成 API Key').closest('a')).toHaveAttribute(
      'href',
      '/settings/api-key-authentication'
    );
    expect(
      within(screen.getByRole('dialog'))
        .getAllByRole('tab')
        .map((tab) => tab.textContent)
    ).toEqual(['通用', 'Codex', 'Claude Code', 'OpenCode']);

    fireEvent.click(screen.getByRole('button', { name: '关 闭' }));
    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));
    expect(await screen.findByLabelText('API Key')).toHaveValue('');
  });
});
