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
  createSettingsMcpInstance: vi.fn(),
  createSettingsMcpTool: vi.fn(),
  createSettingsMcpToolBinding: vi.fn(),
  deleteSettingsMcpGroup: vi.fn(),
  deleteSettingsMcpInstance: vi.fn(),
  deleteSettingsMcpTool: vi.fn(),
  deleteSettingsMcpToolBinding: vi.fn(),
  executeSettingsMcpToolDebug: vi.fn(),
  exportSettingsMcpCatalog: vi.fn(),
  exportSettingsMcpInstanceDirectory: vi.fn(),
  refreshSettingsMcpToolDescription: vi.fn(),
  updateSettingsMcpInstance: vi.fn(),
  updateSettingsMcpMetaToolConfig: vi.fn(),
  updateSettingsMcpTool: vi.fn(),
  updateSettingsMcpToolBinding: vi.fn(),
  upsertSettingsMcpGroup: vi.fn()
}));
const vditorMock = vi.hoisted(() => ({
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
vi.mock('vditor', () => ({
  __esModule: true,
  default: vditorMock.constructor
}));
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
          meta_tool_config: {
            id: 'meta-1',
            workspace_id: 'workspace-1',
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 120,
            list_return_fields: [],
            get_include_mapping_summary: true,
            get_include_interface_summary: true,
            call_default_des_id_policy: 'optional',
            call_high_risk_requires_des_id: true,
            call_validation_error_format: 'json'
          }
        }}
        interfaceCapabilities={capabilities}
      />
    </AppProviders>
  );
}

function renderPanelWithMountedTool({
  operation = 'POST /api/console/apps'
}: {
  operation?: string;
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
          groups: [],
          tools: [
            {
              id: 'tool-record-1',
              workspace_id: 'workspace-1',
              tool_id: 'search_customer',
              name: 'Search customer',
              short_description: 'Search customer',
              full_description: 'Search customer',
              interface_id: 'create_app',
              operation,
              parameter_schema: {},
              result_schema: {},
              input_mapping: {},
              output_mapping: {},
              permission_code: null,
              risk_level: 'low',
              des_id: 'des-1',
              des_id_required: false,
              status: 'enabled',
              revision: 1
            }
          ],
          bindings: [
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
          ],
          meta_tool_config: {
            id: 'meta-1',
            workspace_id: 'workspace-1',
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 120,
            list_return_fields: [],
            get_include_mapping_summary: true,
            get_include_interface_summary: true,
            call_default_des_id_policy: 'optional',
            call_high_risk_requires_des_id: true,
            call_validation_error_format: 'json'
          }
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
  });

  test('keeps mount paths in binding management instead of the tool table', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(within(toolsPanel).queryByPlaceholderText('group_path'))
      .not
      .toBeInTheDocument();
    expect(within(toolsPanel).queryByRole('columnheader', { name: 'group_path' }))
      .not
      .toBeInTheDocument();
    expect(within(toolsPanel).queryByText('/ops/customer')).not.toBeInTheDocument();
    expect(within(toolsPanel).getByRole('columnheader', { name: 'Tool 名称' }))
      .toBeInTheDocument();
    expect(within(toolsPanel).getByRole('columnheader', { name: 'tool_id' }))
      .toBeInTheDocument();
    expect(within(toolsPanel).getByText('Search customer')).toBeInTheDocument();
    expect(within(toolsPanel).getByText('search_customer')).toBeInTheDocument();
    expect(within(toolsPanel).getByRole('columnheader', { name: 'operation' }))
      .toBeInTheDocument();
    expect(within(toolsPanel).getByRole('columnheader', { name: 'interface_id' }))
      .toBeInTheDocument();
    expect(within(toolsPanel).getByText('POST /api/console/apps')).toBeInTheDocument();
    expect(within(toolsPanel).getByText('create_app')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    expect(within(instancesPanel).getByLabelText('挂载路径')).toBeInTheDocument();
    expect(within(instancesPanel).getByRole('columnheader', { name: '挂载路径' }))
      .toBeInTheDocument();
    expect(within(instancesPanel).getAllByText('/ops/customer').length)
      .toBeGreaterThan(0);
  });

  test('falls back to interface id when a stale tool response misses operation', () => {
    renderPanelWithMountedTool({ operation: '' });

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(within(toolsPanel).getAllByText('create_app').length)
      .toBeGreaterThan(0);
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
  }, 30000);

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
  }, 30000);
});
