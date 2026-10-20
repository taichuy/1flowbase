import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, expect, type Mock, vi } from 'vitest';
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
  const React = await import('react');

  type EditorProps = {
    ariaLabel?: string;
    className?: string;
    onChange: (value: string) => void;
    value: string;
  };
  type EditorInstance = (typeof vditorMock.instances)[number];
  type EditorOptions = EditorInstance['options'];
  const createEditor = vditorMock.constructor as unknown as (
    target: HTMLElement,
    options: EditorOptions
  ) => EditorInstance;

  return {
    VditorEditor({
      ariaLabel = 'vditor_editor',
      className,
      onChange,
      value
    }: EditorProps) {
      const mountRef = React.useRef<HTMLDivElement>(null);
      const editorRef = React.useRef<EditorInstance | null>(null);
      const readyRef = React.useRef(false);
      const valueRef = React.useRef(value);
      const onChangeRef = React.useRef(onChange);

      React.useEffect(() => {
        onChangeRef.current = onChange;
      }, [onChange]);

      React.useEffect(() => {
        valueRef.current = value;
        const editor = editorRef.current;
        if (editor && readyRef.current && editor.getValue() !== value) {
          editor.setValue(value);
        }
      }, [value]);

      React.useEffect(() => {
        const mount = mountRef.current;
        if (!mount) return undefined;

        let disposed = false;
        queueMicrotask(() => {
          if (disposed) return;
          const editor = createEditor(mount, {
            mode: 'ir',
            value: valueRef.current,
            after: () => {
              if (disposed) return;
              readyRef.current = true;
              if (editor.getValue() !== valueRef.current) {
                editor.setValue(valueRef.current, true);
              }
            },
            input: (nextValue) => {
              valueRef.current = nextValue;
              onChangeRef.current(nextValue);
            }
          });
          editorRef.current = editor;
        });

        return () => {
          disposed = true;
          readyRef.current = false;
          const editor = editorRef.current;
          editorRef.current = null;
          editor?.destroy();
        };
      }, []);

      return React.createElement('div', {
        ref: mountRef,
        'aria-label': ariaLabel,
        className: ['oneflow-markdown-editor', className]
          .filter(Boolean)
          .join(' ')
      });
    }
  };
});
vi.mock('../../../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

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

export {
  clickSegmentedOption,
  interfaceCapabilities,
  mcpManagementApi,
  publishApplicationApiCapability,
  renderPanel,
  selectAntdOption,
  setFullDescription
};
