import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  setupDirectoryEditorFixture,
  click,
  change,
  mouseDown,
  renderPanel,
  renderPanelWithMountedTool,
  selectAntdOption,
  clickSegmentedOption,
  mcpManagementApi
} from './fixture';

describe('McpManagementPanel', () => {
  setupDirectoryEditorFixture();

  test('falls back to interface id when a stale tool response misses operation', async () => {
    await renderPanelWithMountedTool({ operation: '' });

    await click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(
      within(toolsPanel).getAllByText('create_app').length
    ).toBeGreaterThan(0);
  });

  test('shows step navigation actions only when the adjacent step exists', async () => {
    await renderPanel();

    await click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await click(screen.getByRole('button', { name: /新增/ }));

    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).queryByRole('button', { name: /上一步/ })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: /下一步/ })
    ).toBeInTheDocument();

    await click(within(dialog).getByRole('button', { name: /下一步/ }));
    expect(
      within(dialog).getByRole('button', { name: /上一步/ })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: /下一步/ })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('operation')).toBeInTheDocument();

    await clickSegmentedOption(dialog, 'debug');
    expect(
      within(dialog).getByRole('button', { name: /上一步/ })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: /下一步/ })
    ).not.toBeInTheDocument();
  }, 30000);

  test('places the status selector before description fields in the basic step', async () => {
    await renderPanel();

    await click(screen.getByRole('tab', { name: 'Tool 配置' }));
    await click(screen.getByRole('button', { name: /新增/ }));

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

  test('AC-002 AC-009 AC-010 AC-011 AC-014 edits the local MCP proxy contract without HTTP mapping fields', async () => {
    mcpManagementApi.executeSettingsMcpProxyToolDebug.mockResolvedValue({
      local_arguments: { request: { query: 'status' } },
      remote_arguments: { query: { text: 'status' } },
      upstream_result: { structuredContent: { document: { title: 'Status' } } },
      mapped_result: { structuredContent: { result: { title: 'Status' } } }
    });
    await renderPanelWithMountedTool({ includeBinding: false, proxy: true });

    await click(screen.getByRole('tab', { name: 'Tool 配置' }));
    expect(
      screen.getByRole('columnheader', { name: 'Tool 类型' })
    ).toBeInTheDocument();
    const typeFilter = screen.getByRole('combobox', { name: 'Tool 类型' });
    await mouseDown(typeFilter);
    await selectAntdOption('接口封装');
    await click(screen.getByRole('button', { name: /查\s*询/ }));
    expect(
      screen.queryByRole('row', { name: /Search customer/ })
    ).not.toBeInTheDocument();
    await mouseDown(typeFilter);
    await selectAntdOption('MCP 代理');
    await click(screen.getByRole('button', { name: /查\s*询/ }));
    const toolRow = screen.getByRole('row', { name: /Search customer/ });
    expect(within(toolRow).getByText('MCP 代理')).toBeInTheDocument();
    await click(within(toolRow).getAllByRole('button')[0]);

    const dialog = await screen.findByRole('dialog');
    await clickSegmentedOption(dialog, 'interface');
    expect(within(dialog).getByText('019b-connection')).toBeInTheDocument();
    expect(within(dialog).getByText('search_documents')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('combobox', { name: 'operation' })
    ).not.toBeInTheDocument();
    await mouseDown(
      within(dialog).getByRole('combobox', { name: 'risk_level' })
    );
    await selectAntdOption('medium');

    await clickSegmentedOption(dialog, 'input_mapping');
    expect(within(dialog).getByText('parameter_schema')).toBeInTheDocument();
    await click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    await change(
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

    await clickSegmentedOption(dialog, 'output_mapping');
    expect(within(dialog).getByText('result_schema')).toBeInTheDocument();
    await click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    await change(await within(dialog).findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify({
          type: 'object',
          properties: { local_title: { type: 'string' } }
        })
      }
    });

    await clickSegmentedOption(dialog, 'debug');
    await change(within(dialog).getByLabelText('request.query'), {
      target: { value: 'status' }
    });
    await click(
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

    await click(within(dialog).getByRole('button', { name: '保存' }));
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
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
  }, 30_000);
});
