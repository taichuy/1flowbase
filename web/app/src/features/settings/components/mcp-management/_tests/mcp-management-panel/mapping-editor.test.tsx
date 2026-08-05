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
  }, 30_000);

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
  }, 30_000);

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
  }, 30_000);

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
  }, 30_000);

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
