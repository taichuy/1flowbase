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

  test('AC-003 AC-004 renders persistent Agent CLI configuration commands with the current endpoint and API key', async () => {
    renderPanelWithMountedTool();
    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));
    fireEvent.change(await screen.findByLabelText('API Key'), {
      target: { value: 'test-secret-key' }
    });

    const dialog = screen.getByRole('dialog');
    expect(dialog.closest('.fixed-height-modal')).not.toBeNull();
    expect(
      within(dialog).getByTestId('fixed-height-modal-scroll-body')
    ).toHaveClass(
      'fixed-height-modal__scroll-body',
      'mcp-management__client-configuration-scroll-body'
    );
    const endpoint = `${window.location.origin}/api/mcp/ops_mcp`;

    fireEvent.click(within(dialog).getByRole('tab', { name: 'Codex' }));
    const codexInstallPreview = within(dialog).getByRole('region', {
      name: '安装 / 更新命令预览'
    });
    const codexRemovePreview = within(dialog).getByRole('region', {
      name: '移除命令预览'
    });
    expect(codexInstallPreview).toHaveClass('mcp-client-command-preview');
    expect(codexInstallPreview).toHaveTextContent(endpoint);
    expect(codexInstallPreview).toHaveTextContent('test-secret-key');
    expect(codexInstallPreview).toHaveTextContent('~/.codex/config.toml');
    expect(codexInstallPreview).toHaveTextContent('http_headers');
    expect(codexInstallPreview).toHaveTextContent(
      'Authorization = "Bearer test-secret-key"'
    );
    expect(codexInstallPreview).not.toHaveTextContent('FLOWBASE_MCP_API_KEY');
    expect(codexInstallPreview).not.toHaveTextContent('bearer-token-env-var');
    expect(codexRemovePreview).toHaveTextContent("codex mcp remove 'ops_mcp'");
    expect(codexRemovePreview).not.toHaveTextContent('test-secret-key');
    expect(vditorMock.preview).toHaveBeenCalledWith(
      codexInstallPreview,
      expect.stringContaining('### macOS / Linux Shell'),
      expect.objectContaining({ mode: 'light' })
    );
    const codexInstallMarkdown = vditorMock.preview.mock.calls.find(
      ([previewElement]) => previewElement === codexInstallPreview
    )?.[1];
    expect(codexInstallMarkdown).toMatch(/```bash\n[^\n]+\n```/);
    expect(codexInstallMarkdown).toMatch(/```powershell\n[^\n]+\n```/);
    expect(codexInstallMarkdown).toMatch(/```bat\n[^\n]+\n```/);

    fireEvent.click(within(dialog).getByRole('tab', { name: 'Claude Code' }));
    const claudeCodePanel = within(dialog).getByRole('tabpanel', {
      name: 'Claude Code'
    });
    expect(
      within(claudeCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).toHaveTextContent('claude mcp add --scope user');
    expect(
      within(claudeCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).toHaveTextContent('Authorization: Bearer test-secret-key');
    expect(
      within(claudeCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).not.toHaveTextContent('--env');
    const claudeRemovePreview = within(claudeCodePanel).getByRole('region', {
      name: '移除命令预览'
    });
    expect(claudeRemovePreview).toHaveTextContent(
      "claude mcp remove --scope user 'ops_mcp'"
    );
    expect(claudeRemovePreview).not.toHaveTextContent('test-secret-key');

    fireEvent.click(within(dialog).getByRole('tab', { name: 'OpenCode' }));
    const openCodePanel = within(dialog).getByRole('tabpanel', {
      name: 'OpenCode'
    });
    expect(
      within(openCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).toHaveTextContent('opencode mcp add');
    expect(
      within(openCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).toHaveTextContent('Authorization=Bearer test-secret-key');
    expect(
      within(openCodePanel).getByRole('region', {
        name: '安装 / 更新命令预览'
      })
    ).not.toHaveTextContent('FLOWBASE_MCP_API_KEY');
    expect(openCodePanel).not.toHaveTextContent('opencode mcp remove');
    expect(
      within(openCodePanel).getByText(
        '当前 OpenCode CLI 不支持移除 MCP 配置，请从用户级配置文件中删除当前实例。'
      )
    ).toBeInTheDocument();
  });

  test('AC-005 AC-008 keeps Agent commands copyable but incomplete until an API key is entered', async () => {
    renderPanelWithMountedTool();
    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));

    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('tab', { name: 'Codex' }));

    expect(
      within(dialog).getByText('输入 API Key 后生成可直接执行的命令。')
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('region', { name: '安装 / 更新命令预览' })
    ).not.toBeInTheDocument();
    const removePreview = within(dialog).getByRole('region', {
      name: '移除命令预览'
    });
    expect(removePreview).toHaveTextContent("codex mcp remove 'ops_mcp'");
    expect(removePreview).not.toHaveTextContent('test-secret-key');

    fireEvent.change(within(dialog).getByLabelText('API Key'), {
      target: { value: 'test-secret-key' }
    });
    await waitFor(() => {
      expect(vditorMock.preview).toHaveBeenCalledWith(
        within(dialog).getByRole('region', {
          name: '安装 / 更新命令预览'
        }),
        expect.stringContaining('```bash'),
        expect.objectContaining({ mode: 'light' })
      );
    });
  });

  test('restores and clears a saved MCP client credential without a status badge', async () => {
    mcpManagementApi.fetchSettingsMcpClientCredential.mockResolvedValue({
      saved: true,
      api_key: 'saved-secret-key'
    });
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));

    await waitFor(() => {
      expect(screen.getByLabelText('API Key')).toHaveValue('saved-secret-key');
    });
    expect(screen.queryByText('已加密保存')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /清除认证/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.deleteSettingsMcpClientCredential
      ).toHaveBeenCalledWith('ops_mcp', expect.any(String));
    });
    expect(screen.getByLabelText('API Key')).toHaveValue('');
  });

  test('saves the MCP client API key from the modal footer without a switch', async () => {
    renderPanelWithMountedTool();
    fireEvent.click(screen.getByRole('button', { name: '连接客户端' }));
    fireEvent.change(await screen.findByLabelText('API Key'), {
      target: { value: 'new-secret-key' }
    });

    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    const dialog = screen.getByRole('dialog');
    const footer = dialog.querySelector('.ant-modal-footer');
    expect(footer).not.toBeNull();
    const closeButton = within(footer as HTMLElement).getByRole('button', {
      name: /关\s*闭/
    });
    const clearButton = within(footer as HTMLElement).getByRole('button', {
      name: /清除认证/
    });
    const saveButton = within(footer as HTMLElement).getByRole('button', {
      name: /保存$/
    });
    expect(
      closeButton.compareDocumentPosition(clearButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      clearButton.compareDocumentPosition(saveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(saveButton).toHaveClass('ant-btn-primary');
    fireEvent.click(screen.getByRole('button', { name: /保存$/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.saveSettingsMcpClientCredential
      ).toHaveBeenCalledWith('ops_mcp', 'new-secret-key', expect.any(String));
    });
    expect(screen.queryByText('已加密保存')).not.toBeInTheDocument();
  });

  test('uses Vditor instant rendering mode for full description', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');
    const editor = within(dialog).getByLabelText('full_description');

    expect(editor).toHaveClass('oneflow-markdown-editor');
    await waitFor(() => {
      expect(vditorMock.constructor).toHaveBeenCalled();
    });
    expect(vditorMock.instances[0]?.options.mode).toBe('ir');

    act(() => {
      vditorMock.instances[0]?.options.input?.('Rendered **markdown**');
    });
  });

  test('waits for Vditor readiness before syncing external full description values', async () => {
    const { rerender } = render(
      <MarkdownIrEditor
        ariaLabel="full_description"
        value="Initial description"
      />
    );
    await waitFor(() => {
      expect(vditorMock.instances[0]).toMatchObject({
        setValue: expect.any(Function),
        getValue: expect.any(Function)
      });
    });
    const editor = vditorMock.instances[0];

    rerender(
      <MarkdownIrEditor
        ariaLabel="full_description"
        value="Updated description"
      />
    );

    expect(editor?.getValue).not.toHaveBeenCalled();
    expect(editor?.setValue).not.toHaveBeenCalled();

    act(() => {
      editor?.options.after?.();
    });

    expect(editor?.getValue).toHaveBeenCalled();
    expect(editor?.setValue).toHaveBeenCalledWith('Updated description', true);
  });

  test('shows the selected interface operation in input output and debug steps', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    clickSegmentedOption(dialog, 'interface');
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    clickSegmentedOption(dialog, 'input_mapping');
    expect(visibleTextEntries(dialog, 'POST /api/console/apps').length).toBe(1);
    expect(visibleTextEntries(dialog, 'operationId')).toHaveLength(0);
    expect(visibleTextEntries(dialog, 'risk_level')).toHaveLength(0);
    expect(visibleTextEntries(dialog, 'permission_code')).toHaveLength(0);

    clickSegmentedOption(dialog, 'output_mapping');
    expect(visibleTextEntries(dialog, 'POST /api/console/apps').length).toBe(1);

    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取返回结构' })
    );
    expect(within(dialog).getByDisplayValue('run_id')).toBeInTheDocument();
    expect(within(dialog).queryByDisplayValue('type')).not.toBeInTheDocument();
    expect(
      within(dialog).queryByDisplayValue('required')
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByDisplayValue('properties')
    ).not.toBeInTheDocument();

    clickSegmentedOption(dialog, 'debug');
    expect(visibleTextEntries(dialog, 'POST /api/console/apps').length).toBe(1);
  });

  test('keeps full description in basic and renders debug form JSON results', async () => {
    mcpManagementApi.executeSettingsMcpToolDebug.mockImplementation(
      async (body: { debug_response_mode?: string }) =>
        body.debug_response_mode === 'debug_details'
          ? {
              mcp_arguments: {
                appId: 'app-1'
              },
              interface_arguments: {
                path: {
                  app_id: 'app-1'
                }
              },
              interface_response: {
                data: {
                  run_id: 'run-1',
                  app_id: 'app-1'
                }
              },
              tool_result: {
                run_id: 'run-1'
              }
            }
          : {
              run_id: 'run-1'
            }
    );
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).getByLabelText('full_description')
    ).toBeInTheDocument();
    expect(within(dialog).getAllByText('debug').length).toBeGreaterThan(0);
    expect(within(dialog).queryByText('preview')).not.toBeInTheDocument();

    clickSegmentedOption(dialog, 'interface');
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    clickSegmentedOption(dialog, 'input_mapping');
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    fireEvent.click(await within(dialog).findByText('映射层'));
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('app_id');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));
    fireEvent.change(within(dialog).getByLabelText('mcp_param app_id'), {
      target: { value: 'appId' }
    });

    clickSegmentedOption(dialog, 'output_mapping');
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取返回结构' })
    );

    clickSegmentedOption(dialog, 'debug');
    expect(within(dialog).getByLabelText('appId')).toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('MCP 参数 JSON')
    ).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('appId'), {
      target: { value: 'app-1' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '运行' }));

    await waitFor(() => {
      expect(mcpManagementApi.executeSettingsMcpToolDebug).toHaveBeenCalledWith(
        {
          interface_id: 'create_app',
          mcp_arguments: {
            appId: 'app-1'
          },
          input_mapping: {
            interface_parameters: [
              {
                name: 'app_id',
                field_type: 'string',
                parameter_type: 'url',
                description: 'Application id',
                required: true
              },
              {
                name: 'display_name',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'Display name',
                required: false
              }
            ],
            mappings: [
              {
                interface_param: 'app_id',
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              }
            ]
          },
          output_mapping: {
            type: 'object',
            properties: {
              run_id: {
                type: 'string',
                description: 'Flow run id'
              }
            }
          }
        },
        expect.any(String)
      );
    });

    const debugResult = await within(dialog).findByLabelText('返回值 JSON');
    expect(debugResult).toHaveTextContent('"run_id"');
    expect(debugResult).not.toHaveTextContent('"app_id": "app-1"');
    expect(debugResult).not.toHaveTextContent('"tool_result"');
    expect(debugResult).not.toHaveTextContent('"output_mapping"');

    fireEvent.click(
      within(dialog).getByRole('button', { name: '查看完整内容' })
    );

    await waitFor(() => {
      expect(
        mcpManagementApi.executeSettingsMcpToolDebug
      ).toHaveBeenLastCalledWith(
        {
          interface_id: 'create_app',
          debug_response_mode: 'debug_details',
          mcp_arguments: {
            appId: 'app-1'
          },
          input_mapping: {
            interface_parameters: [
              {
                name: 'app_id',
                field_type: 'string',
                parameter_type: 'url',
                description: 'Application id',
                required: true
              },
              {
                name: 'display_name',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'Display name',
                required: false
              }
            ],
            mappings: [
              {
                interface_param: 'app_id',
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              }
            ]
          },
          output_mapping: {
            type: 'object',
            properties: {
              run_id: {
                type: 'string',
                description: 'Flow run id'
              }
            }
          }
        },
        expect.any(String)
      );
    });

    const debugDetails = await within(dialog).findByLabelText('完整内容 JSON');
    expect(debugDetails).toHaveTextContent('"interface_response"');
    expect(debugDetails).toHaveTextContent('"app_id": "app-1"');
    expect(debugDetails).toHaveTextContent('"tool_result"');
  }, 30000);

  test('renders debug operation and run action in one row without duplicate field-name help text', () => {
    render(
      <McpToolDebugPanel
        operationLabel="POST /api/runtime/models/users/create"
        inputMapping={{
          interface_parameters: [
            {
              name: 'des_id',
              field_type: 'string',
              parameter_type: 'json_body',
              description: 'des_id',
              required: true
            }
          ],
          mappings: [
            {
              interface_param: 'des_id',
              mcp_param: 'des_id',
              description: 'des_id',
              required: true
            }
          ]
        }}
        outputMapping={{ type: 'object' }}
      />
    );

    const header = screen.getByRole('group', { name: '调试操作' });
    expect(
      within(header).getByText('POST /api/runtime/models/users/create')
    ).toBeInTheDocument();
    expect(
      within(header).getByRole('button', { name: '运行' })
    ).toBeInTheDocument();
    expect(
      within(header).getByRole('button', { name: '查看完整内容' })
    ).toBeInTheDocument();
    expect(screen.getAllByText('des_id')).toHaveLength(1);
  });

  test('loads interface descriptors into dedicated input mappings after the explicit mapping action', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).queryByLabelText('des_id_required')
    ).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
    });
    fireEvent.change(within(dialog).getByLabelText('des_id'), {
      target: { value: 'des12345' }
    });
    fireEvent.change(within(dialog).getByLabelText('short_description'), {
      target: { value: 'Create app' }
    });
    await setFullDescription('Create app');
    clickSegmentedOption(dialog, 'interface');
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    clickSegmentedOption(dialog, 'input_mapping');
    expect(
      within(dialog).queryByDisplayValue('app_id')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByDisplayValue('type')).not.toBeInTheDocument();

    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    expect(await within(dialog).findByText('接口层')).toBeInTheDocument();
    expect(within(dialog).getByText('映射层')).toBeInTheDocument();
    expect(within(dialog).getByDisplayValue('app_id')).toBeInTheDocument();
    expect(within(dialog).getByText('URL')).toBeInTheDocument();
    expect(within(dialog).getByText('JSON 请求体')).toBeInTheDocument();
    expect(within(dialog).queryByDisplayValue('type')).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByText('映射层'));
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('app_id');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('display_name');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));
    expect(within(dialog).getByLabelText('mcp_param app_id')).toHaveValue(
      'app_id'
    );
    fireEvent.change(within(dialog).getByLabelText('mcp_param app_id'), {
      target: { value: 'appId' }
    });

    clickSegmentedOption(dialog, 'debug');
    expect(
      within(dialog).queryByText('mcp.get(tool_id)')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('audit_policy')).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          des_id: 'des12345',
          input_mapping: {
            interface_parameters: [
              {
                name: 'app_id',
                field_type: 'string',
                parameter_type: 'url',
                description: 'Application id',
                required: true
              },
              {
                name: 'display_name',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'Display name',
                required: false
              }
            ],
            mappings: [
              {
                interface_param: 'app_id',
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              },
              {
                interface_param: 'display_name',
                mcp_param: 'display_name',
                description: 'Display name',
                required: false
              }
            ]
          }
        }),
        expect.any(String)
      );
    });
    expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
      expect.not.objectContaining({
        des_id_required: expect.any(Boolean)
      }),
      expect.any(String)
    );
    expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
      expect.not.objectContaining({
        audit_policy: expect.anything()
      }),
      expect.any(String)
    );
  }, 30_000);
});
