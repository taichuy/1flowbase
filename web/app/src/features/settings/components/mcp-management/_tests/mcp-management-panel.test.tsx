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
  deleteSettingsMcpUpstreamConnection: vi.fn(),
  deleteSettingsMcpUpstreamConnectionCredentials: vi.fn(),
  discoverSettingsMcpUpstreamConnection: vi.fn(),
  executeSettingsMcpProxyToolDebug: vi.fn(),
  executeSettingsMcpToolDebug: vi.fn(),
  moveSettingsMcpGroup: vi.fn(),
  previewSettingsMcpBundle: vi.fn(),
  importSettingsMcpBundle: vi.fn(),
  importSettingsOfficialMcpBundle: vi.fn(),
  exportSettingsMcpBundle: vi.fn(),
  exportSettingsMcpInstanceBundle: vi.fn(),
  exportSettingsMcpCatalog: vi.fn(),
  fetchSettingsMcpBundleExportDefaults: vi.fn(),
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
  saveSettingsMcpClientCredential: vi.fn(async () => ({ saved: true })),
  saveSettingsMcpUpstreamConnectionCredentials: vi.fn(),
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
import { McpManagementPanel } from '../McpManagementPanel';
import { McpToolDebugPanel } from '../McpToolDebugPanel';
import { MarkdownIrEditor } from '../../../../../shared/ui/markdown-ir-editor/MarkdownIrEditor';

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
    expect(vditorMock.instances.at(-1)).toBeDefined();
  });
  const editor = vditorMock.instances.at(-1);

  act(() => {
    editor!.options.input?.(value);
  });
}

describe('McpManagementPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vditorMock.instances.length = 0;
    window.history.replaceState({}, '', '/settings/mcp-management');
    mcpManagementApi.fetchSettingsMcpClientCredential.mockResolvedValue({
      saved: false
    });
    mcpManagementApi.fetchSettingsMcpBundleExportDefaults.mockResolvedValue({
      minimum_host_version: '0.3.0',
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
      tools: [
        { id: 'runtime_profile', result: 'imported', reason: null },
        {
          id: 'removed_tool',
          result: 'unavailable',
          reason: 'interface_missing'
        }
      ],
      instances: [{ id: 'system', result: 'imported', reason: null }],
      connections: [
        {
          id: '019b5f8f-0000-7000-8000-000000000001',
          result: 'unavailable',
          reason: 'credentials_missing'
        }
      ]
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
      tools: [
        { id: 'runtime_profile', result: 'imported', reason: null },
        {
          id: 'removed_tool',
          result: 'unavailable',
          reason: 'interface_missing'
        }
      ],
      instances: [{ id: 'system', result: 'imported', reason: null }],
      connections: [
        {
          id: '019b5f8f-0000-7000-8000-000000000001',
          result: 'unavailable',
          reason: 'credentials_missing'
        }
      ]
    });
    mcpManagementApi.fetchSettingsOfficialMcpBundles.mockResolvedValue({
      source: {
        source_kind: 'default',
        source_label: '1flowbase official',
        catalog_url: 'https://example.test/mcp/catalog.json'
      },
      entries: [
        {
          organization: 'taichuy',
          bundle_id: '1flowbase_zh_hans',
          latest_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '0.2.6',
          exported_from_system_version: '0.2.5',
          release_tag: 'mcp-taichuy-1flowbase_zh_hans-v1.0.0',
          download_url: 'https://example.test/bundle.zip',
          artifact_sha256: null
        }
      ]
    });
    mcpManagementApi.previewSettingsOfficialMcpBundle.mockImplementation(
      async () => mcpManagementApi.previewSettingsMcpBundle()
    );
    mcpManagementApi.importSettingsOfficialMcpBundle.mockImplementation(
      async () => mcpManagementApi.importSettingsMcpBundle()
    );
  });

  test('uses backend bundle defaults and exports the edited minimum host version', async () => {
    // AC-001 and AC-002: backend default, user-controlled compatibility floor.
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
    const minimumHostVersion = within(dialog).getByLabelText(
      'minimum_host_version'
    );
    await waitFor(() => expect(minimumHostVersion).toHaveValue('0.3.0'));

    fireEvent.change(minimumHostVersion, { target: { value: '0.2.8' } });
    fireEvent.click(
      within(dialog).getByRole('button', { name: /导\s*出/u })
    );

    await waitFor(() => {
      expect(mcpManagementApi.exportSettingsMcpBundle).toHaveBeenCalledWith(
        expect.objectContaining({ minimum_host_version: '0.2.8' }),
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

    fireEvent.click(screen.getByRole('button', { name: '仍然导入' }));
    await waitFor(() => {
      expect(mcpManagementApi.importSettingsMcpBundle).toHaveBeenCalledWith(
        file,
        expect.any(String)
      );
    });
    expect(await screen.findByText('导入完成，但存在警告')).toBeInTheDocument();
  });

  test('previews and imports an official MCP bundle through the backend', async () => {
    renderPanel();

    expect(screen.getByRole('button', { name: /导出$/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /导入$/ }));
    const sourceDialog = await screen.findByRole('dialog', {
      name: '选择配置包来源'
    });
    expect(within(sourceDialog).getByText('官方配置包')).toBeInTheDocument();
    expect(
      await within(sourceDialog).findByText('taichuy/1flowbase_zh_hans')
    ).toBeInTheDocument();

    fireEvent.click(within(sourceDialog).getByRole('button', { name: '预检' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.previewSettingsOfficialMcpBundle
      ).toHaveBeenCalledWith(
        { organization: 'taichuy', bundle_id: '1flowbase_zh_hans' },
        expect.any(String)
      );
    });

    fireEvent.click(await screen.findByRole('button', { name: '仍然导入' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.importSettingsOfficialMcpBundle
      ).toHaveBeenCalledWith(
        { organization: 'taichuy', bundle_id: '1flowbase_zh_hans' },
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
    ).toBeTruthy();

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
    ).toBeTruthy();
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
    ).toBeTruthy();
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
    ).toBeTruthy();
    expect(
      statusField.compareDocumentPosition(shortDescriptionField) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
    expect(
      statusField.compareDocumentPosition(fullDescriptionField) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
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
    expect(codexRemovePreview).toHaveTextContent(
      "codex mcp remove 'ops_mcp'"
    );
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
    ).toBeTruthy();
    expect(
      clearButton.compareDocumentPosition(saveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
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

    expect(editor).toHaveClass('markdown-ir-editor');
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
      expect(vditorMock.instances[0]).toBeDefined();
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

  test('defers Vditor destruction until the pending editor reports ready', async () => {
    const { unmount } = render(
      <MarkdownIrEditor
        ariaLabel="full_description"
        value="Initial description"
      />
    );
    await waitFor(() => {
      expect(vditorMock.instances[0]).toBeDefined();
    });
    const editor = vditorMock.instances[0];

    unmount();

    expect(editor?.destroy).not.toHaveBeenCalled();

    act(() => {
      editor?.options.after?.();
    });

    expect(editor?.destroy).toHaveBeenCalled();
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
  });

  test('uses nested interface descriptors for mapping and debug interface arguments', async () => {
    renderPanel([publishApplicationApiCapability]);

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Publish Application API' }
    });
    fireEvent.change(within(dialog).getByLabelText('des_id'), {
      target: { value: 'des12345' }
    });
    fireEvent.change(within(dialog).getByLabelText('short_description'), {
      target: { value: 'Publish application API' }
    });
    await setFullDescription('Publish application API');
    clickSegmentedOption(dialog, 'interface');
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('publish_application_api');

    clickSegmentedOption(dialog, 'input_mapping');
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );

    expect(
      within(dialog).getByLabelText('field_group mapping')
    ).toBeInTheDocument();
    expect(
      within(dialog).getByLabelText('field_group mapping.input')
    ).toBeInTheDocument();
    expect(
      within(dialog).getByLabelText('field_group mapping.output')
    ).toBeInTheDocument();
    expect(
      within(dialog).getByDisplayValue('query_target')
    ).toBeInTheDocument();
    expect(
      within(dialog).getByDisplayValue('answer_selector')
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByDisplayValue('mapping.input.query_target')
    ).not.toBeInTheDocument();

    fireEvent.click(await within(dialog).findByText('映射层'));
    fireEvent.click(within(dialog).getByRole('button', { name: '全部' }));

    expect(
      within(dialog).getAllByLabelText('field_group mapping.input').length
    ).toBeGreaterThan(0);
    expect(
      within(dialog).getAllByDisplayValue('query_target').length
    ).toBeGreaterThan(0);
    expect(
      within(dialog).getByLabelText('mcp_param mapping.input.query_target')
    ).toHaveValue('mapping.input.query_target');
    expect(within(dialog).getByLabelText('mcp_param des_id')).toHaveValue(
      'des_id'
    );
    expect(within(dialog).getByRole('button', { name: '全部' })).toBeDisabled();

    clickSegmentedOption(dialog, 'debug');
    fireEvent.change(within(dialog).getByLabelText('application_id'), {
      target: { value: 'app-1' }
    });
    fireEvent.click(within(dialog).getByLabelText('api_enabled'));
    fireEvent.change(
      within(dialog).getByLabelText('mapping.input.query_target'),
      {
        target: { value: 'inputs.query' }
      }
    );
    const desIdDebugFields = within(dialog).getAllByLabelText('des_id');
    fireEvent.change(desIdDebugFields[desIdDebugFields.length - 1], {
      target: { value: 'des-1' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '运行' }));

    const debugResult = await within(dialog).findByLabelText('返回值 JSON');
    expect(debugResult).toHaveTextContent('"mapping"');
    expect(debugResult).toHaveTextContent('"input"');
    expect(debugResult).toHaveTextContent('"query_target": "inputs.query"');
    expect(debugResult).not.toHaveTextContent('"mapping.input.query_target"');

    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          des_id: 'des12345',
          input_mapping: {
            interface_parameters: [
              {
                name: 'application_id',
                field_type: 'string',
                parameter_type: 'url',
                description: 'Application id',
                required: true
              },
              {
                name: 'api_enabled',
                field_type: 'boolean',
                parameter_type: 'json_body',
                description: 'API enabled',
                required: true
              },
              {
                name: 'mapping.input.query_target',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'Query target',
                required: true
              },
              {
                name: 'mapping.input.history_target',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'History target',
                required: false
              },
              {
                name: 'mapping.output.answer_selector',
                field_type: 'string',
                parameter_type: 'json_body',
                description: 'Answer selector',
                required: false
              },
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
                interface_param: 'application_id',
                mcp_param: 'application_id',
                description: 'Application id',
                required: true
              },
              {
                interface_param: 'api_enabled',
                mcp_param: 'api_enabled',
                description: 'API enabled',
                required: true
              },
              {
                interface_param: 'mapping.input.query_target',
                mcp_param: 'mapping.input.query_target',
                description: 'Query target',
                required: true
              },
              {
                interface_param: 'mapping.input.history_target',
                mcp_param: 'mapping.input.history_target',
                description: 'History target',
                required: false
              },
              {
                interface_param: 'mapping.output.answer_selector',
                mcp_param: 'mapping.output.answer_selector',
                description: 'Answer selector',
                required: false
              },
              {
                interface_param: 'des_id',
                mcp_param: 'des_id',
                description: 'des_id',
                required: true
              }
            ]
          }
        }),
        expect.any(String)
      );
    });
  });

  test('blocks saving when the input mapping JSON parse view is invalid', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
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

    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    fireEvent.click(await within(dialog).findByText('JSON 解析'));
    await act(async () => {
      await vi.dynamicImportSettled();
    });
    const editor = await within(dialog).findByRole(
      'textbox',
      {
        name: 'input_mapping JSON'
      },
      { timeout: 5000 }
    );
    fireEvent.change(editor, {
      target: { value: '{"interface_parameters":' }
    });

    clickSegmentedOption(dialog, 'debug');
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    expect(mcpManagementApi.createSettingsMcpTool).not.toHaveBeenCalled();
  });

  test('adds the des_id mapping from the mapping layer dropdown option', async () => {
    renderPanel([
      {
        ...interfaceCapabilities[0],
        parameter_descriptors: []
      }
    ]);

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
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
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    fireEvent.click(await within(dialog).findByText('映射层'));
    expect(
      within(dialog).queryByRole('button', { name: /添加 des_id/ })
    ).not.toBeInTheDocument();
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('des_id');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));

    expect(
      within(dialog).getAllByDisplayValue('des_id').length
    ).toBeGreaterThan(1);
    expect(within(dialog).getByLabelText('mcp_param des_id')).toHaveValue(
      'des_id'
    );
    for (const checkbox of within(dialog).getAllByLabelText(
      'required des_id'
    )) {
      expect(checkbox).toBeChecked();
    }
    expect(within(dialog).getByRole('button', { name: '添加' })).toBeDisabled();

    clickSegmentedOption(dialog, 'debug');
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          input_mapping: {
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
          }
        }),
        expect.any(String)
      );
    });
  });

  test('adds all remaining mapping parameters from the mapping layer', async () => {
    renderPanel();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
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
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    fireEvent.click(await within(dialog).findByText('映射层'));
    fireEvent.click(within(dialog).getByRole('button', { name: '全部' }));

    expect(within(dialog).getByLabelText('mcp_param app_id')).toHaveValue(
      'app_id'
    );
    expect(within(dialog).getByLabelText('mcp_param display_name')).toHaveValue(
      'display_name'
    );
    expect(within(dialog).getByLabelText('mcp_param des_id')).toHaveValue(
      'des_id'
    );
    expect(within(dialog).getByRole('button', { name: '全部' })).toBeDisabled();

    clickSegmentedOption(dialog, 'debug');
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
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
              },
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
                interface_param: 'app_id',
                mcp_param: 'app_id',
                description: 'Application id',
                required: true
              },
              {
                interface_param: 'display_name',
                mcp_param: 'display_name',
                description: 'Display name',
                required: false
              },
              {
                interface_param: 'des_id',
                mcp_param: 'des_id',
                description: 'des_id',
                required: true
              }
            ]
          }
        }),
        expect.any(String)
      );
    });
  });

  test('shows des_id once when interface parameters already include it', async () => {
    renderPanel([
      {
        ...interfaceCapabilities[0],
        parameter_descriptors: [
          {
            name: 'application_id',
            field_type: 'string',
            parameter_type: 'json_body' as const,
            description: 'Application id',
            required: true,
            schema: { type: 'string' }
          },
          {
            name: 'application_id',
            field_type: 'string',
            parameter_type: 'json_body' as const,
            description: 'Application id',
            required: true,
            schema: { type: 'string' }
          },
          {
            name: 'des_id',
            field_type: 'string',
            parameter_type: 'json_body' as const,
            description: 'des_id',
            required: true,
            schema: { type: 'string' }
          },
          {
            name: 'des_id',
            field_type: 'string',
            parameter_type: 'json_body' as const,
            description: 'des_id',
            required: true,
            schema: { type: 'string' }
          }
        ]
      }
    ]);

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

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

    const desIdOptions = await screen.findAllByText((text, element) => {
      return Boolean(
        text === 'des_id' && element?.matches('.ant-select-item-option-content')
      );
    });
    const applicationIdOptions = await screen.findAllByText((text, element) => {
      return Boolean(
        text === 'application_id' &&
        element?.matches('.ant-select-item-option-content')
      );
    });

    expect(desIdOptions).toHaveLength(1);
    expect(applicationIdOptions).toHaveLength(2);
  }, 90000);

  test('allows manually adding interface parameters and mappings when descriptors are empty', async () => {
    renderPanel([
      {
        ...interfaceCapabilities[0],
        parameter_descriptors: []
      }
    ]);

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    fireEvent.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
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
    fireEvent.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    expect(
      await within(dialog).findByRole('button', { name: /新增字段/ })
    ).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: /新增字段/ }));
    fireEvent.change(await within(dialog).findByLabelText('field_name 1'), {
      target: { value: 'user_id' }
    });
    fireEvent.change(within(dialog).getByLabelText('field_type user_id'), {
      target: { value: 'string' }
    });
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'parameter_type user_id' })
    );
    await selectAntdOption('URL');
    fireEvent.click(within(dialog).getByLabelText('required user_id'));

    fireEvent.click(within(dialog).getByText('映射层'));
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('user_id');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));
    fireEvent.change(within(dialog).getByLabelText('mcp_param user_id'), {
      target: { value: 'userId' }
    });
    fireEvent.change(within(dialog).getByLabelText('description user_id'), {
      target: { value: 'User id' }
    });

    clickSegmentedOption(dialog, 'debug');
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          input_mapping: {
            interface_parameters: [
              {
                name: 'user_id',
                field_type: 'string',
                parameter_type: 'url',
                description: '',
                required: true
              }
            ],
            mappings: [
              {
                interface_param: 'user_id',
                mcp_param: 'userId',
                description: 'User id',
                required: true
              }
            ]
          }
        }),
        expect.any(String)
      );
    });
  }, 90000);
});
