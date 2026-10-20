import { describe, test, expect } from 'vitest';
import {
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
} from './client-and-tool-editor.fixtures';
import { MarkdownIrEditor } from '../../../../../../shared/ui/markdown-ir-editor/MarkdownIrEditor';
import { McpToolDebugPanel } from '../../McpToolDebugPanel';
import { act, screen, waitFor, within } from '@testing-library/react';
import { i18nText } from '../../../../../../shared/i18n/text';

describe('McpManagementPanel', () => {
  test('AC-001 keeps the full-description Vditor outline closed by default', async () => {
    await renderPanel();

    await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await domEvents.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');
    const editor = within(dialog).getByLabelText('full_description');

    expect(editor).toHaveClass('oneflow-markdown-editor');
    await waitFor(() => {
      expect(vditorMock.constructor).toHaveBeenCalled();
    });
    expect(vditorMock.instances[0]?.options.mode).toBe('ir');
    expect(vditorMock.instances[0]?.options.outline).toEqual({
      enable: false,
      position: 'left'
    });
    expect(vditorMock.instances[0]?.options.value).toBe('');

    await act(async () => {
      vditorMock.instances[0]?.options.input?.('Rendered **markdown**');
    });
  });

  test('AC-001 saves an interface wrapper from input mapping when full description is empty', async () => {
    await renderPanelWithMountedTool({ fullDescription: '' });

    await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolRow = screen.getByRole('row', { name: /Search customer/ });
    await domEvents.click(within(toolRow).getAllByRole('button')[0]);

    const dialog = await screen.findByRole('dialog');
    await clickSegmentedOption(dialog, 'input_mapping');
    await domEvents.click(
      within(dialog).getByRole('button', {
        name: i18nText('settings', 'auto.save')
      })
    );

    await waitFor(() => {
      expect(mcpManagementApi.updateSettingsMcpTool).toHaveBeenCalledWith(
        'search_customer',
        expect.objectContaining({ full_description: '' }),
        expect.any(String)
      );
    });
  });

  test('waits for Vditor readiness before syncing external full description values', async () => {
    const { rerender } = await renderSettled(
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

    await act(async () => {
      editor?.options.after?.();
    });

    expect(editor?.getValue).toHaveBeenCalled();
    expect(editor?.setValue).toHaveBeenCalledWith('Updated description', true);
  });

  test('shows the selected interface operation in input output and debug steps', async () => {
    await renderPanel();

    await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await domEvents.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    await clickSegmentedOption(dialog, 'interface');
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    await clickSegmentedOption(dialog, 'input_mapping');
    expect(visibleTextEntries(dialog, 'POST /api/console/apps').length).toBe(1);
    expect(visibleTextEntries(dialog, 'operationId')).toHaveLength(0);
    expect(visibleTextEntries(dialog, 'risk_level')).toHaveLength(0);
    expect(visibleTextEntries(dialog, 'permission_code')).toHaveLength(0);

    await clickSegmentedOption(dialog, 'output_mapping');
    expect(visibleTextEntries(dialog, 'POST /api/console/apps').length).toBe(1);

    await domEvents.click(
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

    await clickSegmentedOption(dialog, 'debug');
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
    await renderPanel();

    await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await domEvents.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).getByLabelText('full_description')
    ).toBeInTheDocument();
    expect(within(dialog).getAllByText('debug').length).toBeGreaterThan(0);
    expect(within(dialog).queryByText('preview')).not.toBeInTheDocument();

    await clickSegmentedOption(dialog, 'interface');
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    await clickSegmentedOption(dialog, 'input_mapping');
    await domEvents.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    await domEvents.click(await within(dialog).findByText('映射层'));
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('app_id');
    await domEvents.click(within(dialog).getByRole('button', { name: '添加' }));
    await domEvents.change(within(dialog).getByLabelText('mcp_param app_id'), {
      target: { value: 'appId' }
    });

    expect(within(dialog).getByLabelText('default_value app_id')).toHaveValue(
      ''
    );
    expect(within(dialog).getByLabelText('hidden app_id')).not.toBeChecked();

    await clickSegmentedOption(dialog, 'output_mapping');
    await domEvents.click(
      within(dialog).getByRole('button', { name: '获取返回结构' })
    );

    await clickSegmentedOption(dialog, 'debug');
    expect(within(dialog).getByLabelText('appId')).toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('MCP 参数 JSON')
    ).not.toBeInTheDocument();
    await domEvents.change(within(dialog).getByLabelText('appId'), {
      target: { value: 'app-1' }
    });
    await domEvents.click(within(dialog).getByRole('button', { name: '运行' }));

    await waitFor(() => {
      expect(mcpManagementApi.executeSettingsMcpToolDebug).toHaveBeenCalledWith(
        {
          interface_id: 'create_app',
          mcp_arguments: {
            appId: 'app-1'
          },
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
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
            ]),
            mappings: [
              {
                interface_param: 'app_id',
                default_value: null,
                hidden: false,
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              }
            ]
          }),
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

    await domEvents.click(
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
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
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
            ]),
            mappings: [
              {
                interface_param: 'app_id',
                default_value: null,
                hidden: false,
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              }
            ]
          }),
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

  test('renders debug operation and run action in one row without duplicate field-name help text', async () => {
    await renderSettled(
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
    await renderPanel();

    await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await domEvents.click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).queryByLabelText('des_id_required')
    ).not.toBeInTheDocument();
    await domEvents.change(within(dialog).getByLabelText('name'), {
      target: { value: 'Create App' }
    });
    await domEvents.change(within(dialog).getByLabelText('des_id'), {
      target: { value: 'des12345' }
    });
    await domEvents.change(within(dialog).getByLabelText('short_description'), {
      target: { value: 'Create app' }
    });
    await setFullDescription('Create app');
    await clickSegmentedOption(dialog, 'interface');
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'operation' })
    );
    await selectAntdOption('create_app');

    await clickSegmentedOption(dialog, 'input_mapping');
    expect(
      within(dialog).queryByDisplayValue('app_id')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByDisplayValue('type')).not.toBeInTheDocument();

    await domEvents.click(
      within(dialog).getByRole('button', { name: '获取接口参数' })
    );
    for (const [name, fieldType] of [
      ['des_id', 'string'],
      ['max_inline_chars', 'integer'],
      ['response_fields', 'array<string>']
    ]) {
      const row = within(dialog).getByLabelText(`call_parameter ${name}`);
      expect(within(row).getByLabelText(`field_type ${name}`)).toHaveValue(
        fieldType
      );
      expect(within(row).getByLabelText(`parameter_type ${name}`)).toHaveValue(
        'JSON 请求体'
      );
      expect(within(row).getByLabelText(`required ${name}`)).not.toBeChecked();
    }
    expect(await within(dialog).findByText('接口层')).toBeInTheDocument();
    expect(within(dialog).getByText('映射层')).toBeInTheDocument();
    expect(within(dialog).getByDisplayValue('app_id')).toBeInTheDocument();
    expect(within(dialog).getByText('URL')).toBeInTheDocument();
    expect(within(dialog).getByText('JSON 请求体')).toBeInTheDocument();
    expect(within(dialog).queryByDisplayValue('type')).not.toBeInTheDocument();

    await domEvents.click(within(dialog).getByText('映射层'));
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('app_id');
    await domEvents.click(within(dialog).getByRole('button', { name: '添加' }));
    await domEvents.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('display_name');
    await domEvents.click(within(dialog).getByRole('button', { name: '添加' }));
    expect(within(dialog).getByLabelText('mcp_param app_id')).toHaveValue(
      'app_id'
    );
    await domEvents.change(within(dialog).getByLabelText('mcp_param app_id'), {
      target: { value: 'appId' }
    });

    for (const name of ['app_id', 'display_name']) {
      expect(
        within(dialog).getByLabelText(`default_value ${name}`)
      ).toHaveValue('');
      expect(within(dialog).getByLabelText(`hidden ${name}`)).not.toBeChecked();
    }

    await clickSegmentedOption(dialog, 'debug');
    expect(
      within(dialog).queryByText('mcp_get(tool_id)')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('audit_policy')).not.toBeInTheDocument();
    await domEvents.click(
      within(dialog).getByRole('button', {
        name: i18nText('settings', 'auto.save')
      })
    );

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          des_id: 'des12345',
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
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
            ]),
            mappings: [
              {
                interface_param: 'app_id',
                default_value: null,
                hidden: false,
                mcp_param: 'appId',
                description: 'Application id',
                required: true
              },
              {
                interface_param: 'display_name',
                default_value: null,
                hidden: false,
                mcp_param: 'display_name',
                description: 'Display name',
                required: false
              }
            ]
          })
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
