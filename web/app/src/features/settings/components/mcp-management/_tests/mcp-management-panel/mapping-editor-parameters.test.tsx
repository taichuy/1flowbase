import { describe, test, expect } from 'vitest';
import {
  clickSegmentedOption,
  fireEvent,
  interfaceCapabilities,
  mcpManagementApi,
  renderPanel,
  screen,
  selectAntdOption,
  setFullDescription,
  waitFor,
  within
} from './mapping-editor.fixtures';

describe('McpManagementPanel', () => {
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
    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
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
            mappings: expect.arrayContaining([
              {
                interface_param: 'app_id',
                default_value: null,
                hidden: false,
                mcp_param: 'app_id',
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
            ])
          })
        }),
        expect.any(String)
      );
    });
  }, 30_000);

  test('shows des_id once and preserves selectable repeated interface descriptors', async () => {
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
    fireEvent.click(applicationIdOptions[1]);
    fireEvent.click(within(dialog).getByRole('button', { name: /^添\s*加$/ }));
    expect(
      within(dialog).getByLabelText('mcp_param application_id')
    ).toHaveValue('application_id');
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
    fireEvent.change(await within(dialog).findByLabelText('field_name 4'), {
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
    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mcpManagementApi.createSettingsMcpTool).toHaveBeenCalledWith(
        expect.objectContaining({
          input_mapping: expect.objectContaining({
            interface_parameters: expect.arrayContaining([
              {
                name: 'user_id',
                field_type: 'string',
                parameter_type: 'url',
                description: '',
                required: true
              }
            ]),
            mappings: [
              {
                interface_param: 'user_id',
                default_value: null,
                hidden: false,
                mcp_param: 'userId',
                description: 'User id',
                required: true
              }
            ]
          })
        }),
        expect.any(String)
      );
    });
  }, 90000);
});
