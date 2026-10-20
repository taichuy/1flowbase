import { describe, test, expect } from 'vitest';
import {
  clickSegmentedOption,
  interfaceCapabilities,
  mcpManagementApi,
  publishApplicationApiCapability,
  renderPanel,
  selectAntdOption,
  setFullDescription
} from './mapping-editor.fixtures';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';

describe('McpManagementPanel', () => {
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
    fireEvent.click(within(dialog).getByRole('button', { name: '运行' }));

    const debugResult = await within(dialog).findByLabelText('返回值 JSON');
    expect(debugResult).toHaveTextContent('"mapping"');
    expect(debugResult).toHaveTextContent('"input"');
    expect(debugResult).toHaveTextContent('"query_target": "inputs.query"');
    expect(debugResult).not.toHaveTextContent('"mapping.input.query_target"');

    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          des_id: 'des12345',
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
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
              }
            ]),
            mappings: expect.arrayContaining([
              {
                interface_param: 'application_id',
                default_value: null,
                hidden: false,
                mcp_param: 'application_id',
                description: 'Application id',
                required: true
              },
              {
                interface_param: 'api_enabled',
                default_value: null,
                hidden: false,
                mcp_param: 'api_enabled',
                description: 'API enabled',
                required: true
              },
              {
                interface_param: 'mapping.input.query_target',
                default_value: null,
                hidden: false,
                mcp_param: 'mapping.input.query_target',
                description: 'Query target',
                required: true
              },
              {
                interface_param: 'mapping.input.history_target',
                default_value: null,
                hidden: false,
                mcp_param: 'mapping.input.history_target',
                description: 'History target',
                required: false
              },
              {
                interface_param: 'mapping.output.answer_selector',
                default_value: null,
                hidden: false,
                mcp_param: 'mapping.output.answer_selector',
                description: 'Answer selector',
                required: false
              }
            ])
          })
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
    expect(
      await within(dialog).findByText('请输入合法 JSON')
    ).toBeInTheDocument();

    clickSegmentedOption(dialog, 'debug');
    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    expect(
      await screen.findByText(
        '请填写符合参数类型的默认值；隐藏的必填参数必须配置默认值。'
      )
    ).toBeInTheDocument();
    expect(mcpManagementApi.createSettingsMcpTool).not.toHaveBeenCalled();
  }, 30_000);

  test('shows optional MCP controls without synthesizing business mappings', async () => {
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
    for (const name of ['des_id', 'max_inline_chars', 'response_fields']) {
      const row = within(dialog).getByLabelText(`call_parameter ${name}`);
      expect(row).toHaveClass('mcp-input-mapping-editor__row');
      expect(within(row).getByLabelText(`required ${name}`)).not.toBeChecked();
    }
    fireEvent.click(await within(dialog).findByText('映射层'));
    expect(
      within(dialog).queryByLabelText('open_call_parameter max_inline_chars')
    ).not.toBeInTheDocument();
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'interface_param' })
    );
    await selectAntdOption('max_inline_chars');
    fireEvent.click(within(dialog).getByRole('button', { name: '添加' }));
    expect(
      within(dialog).getByLabelText('mcp_param max_inline_chars')
    ).toHaveValue('max_inline_chars');
    expect(
      within(dialog).getByLabelText('description max_inline_chars')
    ).toHaveValue('返回字符预算；正整数，留空使用 Tool 默认值，例如 4000。');
    expect(
      within(dialog).queryByLabelText('mcp_param des_id')
    ).not.toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '添加' })).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: '全部' })).toBeEnabled();

    clickSegmentedOption(dialog, 'debug');
    expect(
      within(dialog).queryByLabelText('max_inline_chars')
    ).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
              expect.objectContaining({
                name: 'max_inline_chars',
                source: { kind: 'mcp_call' }
              })
            ]),
            mappings: [
              {
                interface_param: 'max_inline_chars',
                default_value: null,
                hidden: false,
                mcp_param: 'max_inline_chars',
                description:
                  '返回字符预算；正整数，留空使用 Tool 默认值，例如 4000。',
                required: false,
                source: { kind: 'mcp_call', path: 'max_inline_chars' }
              }
            ]
          })
        }),
        expect.any(String)
      );
    });
  }, 30_000);
});
