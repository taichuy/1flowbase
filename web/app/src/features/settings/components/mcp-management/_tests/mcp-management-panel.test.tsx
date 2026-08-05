/* eslint-disable @typescript-eslint/no-unused-vars -- shared MCP panel fixture inventory is intentionally broader than each scenario file. */
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
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

vi.mock('../../../api/mcp-management', () => mcpManagementApi);
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

import { AppProviders } from '../../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { McpManagementPanel } from '../McpManagementPanel';

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

  test('shows the backend system version and exports without a client-selected host floor', async () => {
    mcpManagementApi.exportSettingsMcpBundle.mockResolvedValue({
      blob: new Blob(['bundle'], { type: 'application/zip' }),
      filename: 'mcp-bundle.zip'
    });
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value: vi.fn(() => 'blob:mcp-bundle')
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value: vi.fn()
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    renderPanel();

    fireEvent.click(screen.getByRole('button', { name: /导出$/ }));
    const dialog = await screen.findByRole('dialog', {
      name: '导出 MCP 配置包'
    });
    expect(within(dialog).queryByLabelText('minimum_host_version')).not.toBeInTheDocument();
    expect(await within(dialog).findByText(/0.3.0/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /导\s*出/u }));

    await waitFor(() => {
      expect(mcpManagementApi.exportSettingsMcpBundle).toHaveBeenCalledWith(
        expect.not.objectContaining({
          minimum_host_version: expect.anything()
        }),
        expect.any(String)
      );
    });
  });

  test('previews an MCP bundle, warns for older source and imports the remaining items', async () => {
    // AC-005, AC-007, AC-009 and AC-010.
    renderPanel();

    const file = new File(['bundle'], 'taichuy-bundle.zip', {
      type: 'application/zip'
    });
    fireEvent.change(screen.getByLabelText('选择 MCP 配置包'), {
      target: { files: [file] }
    });

    expect(
      await screen.findByText('配置包来自较低版本的 1flowbase')
    ).toBeInTheDocument();
    expect(screen.getByText('removed_tool')).toBeInTheDocument();
    expect(screen.getByText('接口不存在')).toBeInTheDocument();
    expect(
      screen.getByText('019b5f8f-0000-7000-8000-000000000001')
    ).toBeInTheDocument();
    expect(screen.getByText('缺少凭据')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '确认导入并覆盖' }));
    await waitFor(() => {
      expect(mcpManagementApi.importSettingsMcpBundle).toHaveBeenCalledWith(
        file,
        expect.any(String)
      );
    });
    expect(await screen.findByText('导入完成，但存在警告')).toBeInTheDocument();
  });

  test('reuses the local template library and import flow in MCP Management', async () => {
    renderPanel();

    expect(screen.getByRole('button', { name: /导出$/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /导入$/ }));
    const sourceDialog = await screen.findByRole('dialog', {
      name: '选择配置包来源'
    });
    expect(
      await within(sourceDialog).findByText('taichuy/1flowbase_zh_hans')
    ).toBeInTheDocument();

    const bundleRow = within(sourceDialog).getByRole('row', {
      name: /taichuy\/1flowbase_zh_hans/
    });
    fireEvent.click(within(bundleRow).getByRole('button', { name: '导入' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.previewSettingsMcpTemplateLibraryBundle
      ).toHaveBeenCalledWith(
        'taichuy',
        '1flowbase_zh_hans',
        {},
        expect.any(String)
      );
    });

    fireEvent.click(
      await screen.findByRole('button', { name: '确认导入并覆盖' })
    );
    await waitFor(() => {
      expect(
        mcpManagementApi.importSettingsMcpTemplateLibraryBundle
      ).toHaveBeenCalledWith(
        'taichuy',
        '1flowbase_zh_hans',
        {},
        expect.any(String)
      );
    });
  });

  test('keeps mount paths in binding management instead of the tool table', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(
      within(toolsPanel).queryByPlaceholderText('group_path')
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).queryByRole('columnheader', { name: 'group_path' })
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).queryByText('/ops/customer')
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'Tool 名称' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'tool_id' })
    ).toBeInTheDocument();
    expect(within(toolsPanel).getByText('Search customer')).toBeInTheDocument();
    expect(within(toolsPanel).getByText('search_customer')).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'operation' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'Tool 类型' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: '执行来源' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByText('POST /api/console/apps')
    ).toBeInTheDocument();
    expect(within(toolsPanel).getByText('create_app')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    expect(
      within(instancesPanel).queryByLabelText('挂载路径')
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('columnheader', { name: '挂载路径' })
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByText('/ops/customer')
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const modalShell = dialog.closest('.ant-modal');
    const modalScrollBody = screen.getByTestId(
      'fixed-height-modal-scroll-body'
    );

    expect(modalShell).toHaveStyle({ width: '840px' });
    expect(modalScrollBody).toHaveClass('mcp-management__directory-modal');
    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('路径')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('combobox', { name: '路径' })
    ).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('挂载路径')).not.toBeVisible();
    expect(within(dialog).getByRole('tree')).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(
      within(dialog).getByRole('heading', { name: '新建 Tool 挂载' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('路径')).not.toBeVisible();
    expect(within(dialog).getByLabelText('挂载路径')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('columnheader', { name: '挂载路径' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole('columnheader', { name: 'display_alias' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getAllByLabelText('编辑 Tool 挂载').length
    ).toBeGreaterThan(0);
  });

  test('shows instance name and instance_id in separate columns with matching action icons', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    expect(
      within(instancesPanel)
        .getAllByRole('columnheader')
        .slice(0, 2)
        .map((header) => header.textContent)
    ).toEqual(['instance_id', '实例名称']);

    const directoryEditorButton = within(instancesPanel).getByRole('button', {
      name: '目录编辑'
    });
    const editButton = within(instancesPanel).getByRole('button', {
      name: '编辑'
    });

    expect(
      editButton.compareDocumentPosition(directoryEditorButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    expect(
      directoryEditorButton.querySelector('.anticon-setting')
    ).toBeInTheDocument();
    expect(editButton.querySelector('.anticon-edit')).toBeInTheDocument();
  });

  test('keeps three primary instance actions and moves the rest into more actions', async () => {
    mcpManagementApi.exportSettingsMcpInstanceBundle.mockResolvedValue({
      blob: new Blob(['bundle'], { type: 'application/zip' }),
      filename: 'mcp-instance-ops_mcp.zip'
    });
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value: vi.fn(() => 'blob:mcp-instance')
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value: vi.fn()
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    renderPanelWithMountedTool();

    const instancesPanel = screen.getByRole('tabpanel', {
      name: 'MCP 实例'
    });
    expect(
      within(instancesPanel).getByRole('button', { name: '编辑' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).getByRole('button', { name: '连接客户端' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('button', { name: '目录发现配置' })
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('button', { name: '导出此实例' })
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    expect(
      within(menu).getByRole('menuitem', { name: /目录发现配置$/ })
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: /复制实例$/ })
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: /删除$/ })
    ).toBeInTheDocument();
    fireEvent.click(
      within(menu).getByRole('menuitem', { name: /导出此实例$/ })
    );
    const dialog = await screen.findByRole('dialog', {
      name: '导出 MCP 实例配置包'
    });
    expect(within(dialog).getByLabelText('bundle_id')).toHaveValue('ops_mcp');

    fireEvent.click(within(dialog).getByRole('button', { name: '导出此实例' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.exportSettingsMcpInstanceBundle
      ).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({ bundle_id: 'ops_mcp' }),
        expect.any(String)
      );
    });
  });

  test('copies an instance after requiring a new id and name', async () => {
    mcpManagementApi.copySettingsMcpInstance.mockResolvedValue({
      id: 'instance-copy-id',
      instance_id: 'ops_mcp_copy',
      name: 'Ops MCP Copy',
      description_short: 'Mounted instance',
      status: 'draft',
      default_entry_path: '/ops'
    });
    renderPanelWithMountedTool();

    const instancesPanel = screen.getByRole('tabpanel', {
      name: 'MCP 实例'
    });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    fireEvent.click(within(menu).getByRole('menuitem', { name: /复制实例$/ }));
    const copyDialog = await screen.findByRole('dialog', {
      name: '复制 MCP 实例'
    });
    fireEvent.change(within(copyDialog).getByLabelText('instance_id'), {
      target: { value: 'ops_mcp_copy' }
    });
    fireEvent.change(within(copyDialog).getByLabelText('实例名称'), {
      target: { value: 'Ops MCP Copy' }
    });
    fireEvent.click(
      within(copyDialog).getByRole('button', { name: /复\s*制/ })
    );
    await waitFor(() => {
      expect(mcpManagementApi.copySettingsMcpInstance).toHaveBeenCalledWith(
        'ops_mcp',
        { instance_id: 'ops_mcp_copy', name: 'Ops MCP Copy' },
        expect.any(String)
      );
    });
  });

  test('hides the edit binding selector when there are no existing tool bindings', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).queryAllByLabelText('编辑 Tool 挂载')).toHaveLength(
      0
    );
    expect(within(dialog).getByLabelText('挂载路径')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
  });

  test('localizes directory editor field labels while keeping tool_id raw', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    expect(within(dialog).getByLabelText('路径')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('显示名称')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('简短描述')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('启用')).toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('display_name')
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('description_short')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('enabled')).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('可见')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('显示别名')).not.toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('display_alias')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('visible')).not.toBeInTheDocument();
  });

  test('does not expose or preserve display alias when saving a tool binding', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    const rootLabel = within(dialog).getByText('Ops MCP /');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).queryByLabelText('显示别名')).not.toBeInTheDocument();

    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'tool_id' })
    );
    await selectAntdOption('Search customer');
    fireEvent.click(within(dialog).getByRole('button', { name: /保存/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.createSettingsMcpToolBinding
      ).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({
          tool_id: 'search_customer',
          display_alias: null
        }),
        expect.any(String)
      );
    });
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
  });

  test('keeps the directory editor open and selects the saved group', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(dialog).getByText('Ops MCP /');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));
    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Customer Ops' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: /保存/ }));

    await waitFor(() => {
      expect(mcpManagementApi.upsertSettingsMcpGroup).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({
          display_name: 'Customer Ops'
        }),
        expect.any(String)
      );
    });
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
  });

  test('exposes explicit creation actions beside the directory tree', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: '新增' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getByRole('heading', { name: '新建分组' })
    ).toBeInTheDocument();
  });

  test('starts the directory tree fully collapsed', () => {
    renderPanelWithMountedTool({ includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const tree = within(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).getByRole('tree');
    expect(within(tree).getAllByRole('treeitem')[0]).toHaveAttribute(
      'aria-expanded',
      'false'
    );
    expect(within(tree).queryByText('ops')).not.toBeInTheDocument();
  });

  test('starts a child path from the selected group', () => {
    renderPanelWithMountedTool({ includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    fireEvent.click(within(dialog).getByText('ops'));
    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));

    expect(within(dialog).getByText(/新增至父目录/)).toBeInTheDocument();
    expect(within(dialog).getByLabelText('路径')).toHaveValue('/ops/');
    expect(within(dialog).getByText('目标目录： /ops')).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '取消子分组新建' })
    ).toBeInTheDocument();
  });

  test('mounts a Tool under the selected group without losing tree selection', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);

    const groupLabel = within(dialog).getByText('ops');
    fireEvent.click(groupLabel);
    await waitFor(() => {
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
    });

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(
      within(dialog).getByRole('heading', { name: '新建 Tool 挂载' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/ops');
    expect(
      within(dialog).getByText('ops').closest('.ant-tree-node-content-wrapper')
    ).toHaveClass('ant-tree-node-selected');
  });

  test('edits a group directly without changing the selected directory', () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);

    const rootLabel = within(dialog).getByText('Ops MCP /');
    const rootNode = rootLabel.closest('.ant-tree-node-content-wrapper');
    const groupLabel = within(dialog).getByText('ops');
    const groupNode = groupLabel.closest('.ant-tree-node-content-wrapper');
    expect(rootNode).toHaveClass('ant-tree-node-selected');
    expect(groupNode).not.toHaveClass('ant-tree-node-selected');

    fireEvent.mouseEnter(groupNode as HTMLElement);
    fireEvent.click(
      within(groupNode as HTMLElement).getByRole('button', { name: '编辑' })
    );

    expect(rootNode).toHaveClass('ant-tree-node-selected');
    expect(groupNode).not.toHaveClass('ant-tree-node-selected');
    const status = dialog.querySelector(
      '.mcp-management__directory-editor-status'
    );
    expect(status).toBeInstanceOf(HTMLElement);
    expect(status).toHaveTextContent('已保存');
    expect(status).toHaveTextContent('分组');
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue('ops');
  });

  test('shows unsaved after editing and returns to saved after saving', async () => {
    mcpManagementApi.upsertSettingsMcpGroup.mockResolvedValue(undefined);
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);
    fireEvent.click(within(dialog).getByText('ops'));

    const status = dialog.querySelector(
      '.mcp-management__directory-editor-status'
    );
    expect(status).toHaveTextContent('已保存');

    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Updated ops' }
    });
    expect(status).toHaveTextContent('未保存');

    fireEvent.click(within(dialog).getByRole('button', { name: '保存分组' }));
    await waitFor(() => expect(status).toHaveTextContent('已保存'));

    fireEvent.click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '目录编辑' })
      ).not.toBeInTheDocument();
    });
    expect(screen.queryByText('放弃未保存的更改？')).not.toBeInTheDocument();
  });

  test('selects a group node and opens that group for editing', () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);
    fireEvent.click(within(dialog).getByText('ops'));

    expect(
      within(dialog).getByText('ops').closest('.ant-tree-node-content-wrapper')
    ).toHaveClass('ant-tree-node-selected');
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue('ops');
    expect(
      within(dialog).getByRole('button', { name: '保存分组' })
    ).toBeInTheDocument();
  });

  test('closes the directory editor after an empty Tool mount validation failure', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    const visibleSwitch = within(dialog).getByRole('switch', { name: '可见' });
    fireEvent.click(visibleSwitch);
    fireEvent.click(visibleSwitch);
    fireEvent.click(
      within(dialog).getByRole('button', { name: '保存 Tool 挂载' })
    );
    await waitFor(() => {
      expect(within(dialog).getByLabelText('tool_id')).toHaveAttribute(
        'aria-invalid',
        'true'
      );
    });

    fireEvent.click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );

    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '目录编辑' })
      ).not.toBeInTheDocument();
    });
  });
});
