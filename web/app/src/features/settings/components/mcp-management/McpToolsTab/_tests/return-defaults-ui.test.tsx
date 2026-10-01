import { App } from 'antd';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConsoleMcpCatalog, ConsoleMcpTool } from '@1flowbase/api-client';
import { McpToolsTab } from '../../McpToolsTab';

const api = vi.hoisted(() => ({ updateSettingsMcpTool: vi.fn() }));
vi.mock('../../../../api/mcp-management', async (original) => ({
  ...(await original<typeof import('../../../../api/mcp-management')>()),
  ...api
}));
vi.mock(
  '../../../../../../shared/ui/markdown-ir-editor/MarkdownIrEditor',
  () => ({
    MarkdownIrEditor: ({
      ariaLabel,
      value
    }: {
      ariaLabel: string;
      value: string;
    }) => <textarea aria-label={ariaLabel} value={value ?? ''} readOnly />
  })
);

const tool: ConsoleMcpTool = {
  id: 'tool',
  workspace_id: 'workspace',
  tool_id: 'read_tab',
  name: 'Read tab',
  short_description: 'Read tab content',
  full_description: '',
  execution_target: { kind: 'assistant_client', capability_code: 'read_tab' },
  operation: 'read_tab',
  parameter_schema: { type: 'object', properties: {} },
  result_schema: { type: 'object', properties: {} },
  input_mapping: {},
  output_mapping: {},
  max_inline_chars: 1200,
  response_fields: ['/title'],
  permission_code: null,
  risk_level: 'low',
  des_id: 'abcdefgh',
  des_id_required: false,
  status: 'enabled',
  availability_status: 'available',
  availability_reason: null,
  revision: 1,
  managed_by: null
};

async function openEditor() {
  const catalog: ConsoleMcpCatalog = {
    instances: [],
    groups: [],
    tools: [tool],
    bindings: [],
    discovery_policies: []
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: {
            queries: { retry: false },
            mutations: { retry: false }
          }
        })
      }
    >
      <App>
        <McpToolsTab canManage catalog={catalog} interfaceCapabilities={[]} />
      </App>
    </QueryClientProvider>
  );
  const row = await screen.findByRole('row', { name: /Read tab/ });
  fireEvent.click(within(row).getAllByRole('button')[0]);
  return screen.findByRole('dialog');
}

describe('MCP return default editor', () => {
  beforeEach(() => {
    api.updateSettingsMcpTool.mockReset();
    api.updateSettingsMcpTool.mockResolvedValue(tool);
    window.history.replaceState(null, '', '/settings/mcp-management');
  });

  it('preserves configured defaults when saving from basic', async () => {
    const dialog = await openEditor();
    fireEvent.click(within(dialog).getByText('OK'));
    await waitFor(() =>
      expect(api.updateSettingsMcpTool).toHaveBeenCalledWith(
        'read_tab',
        expect.objectContaining({
          max_inline_chars: 1200,
          response_fields: ['/title']
        }),
        expect.any(String)
      )
    );
  });

  it('renders both defaults as single-line inputs in basic and saves overrides', async () => {
    const dialog = await openEditor();
    const budget = within(dialog).getByRole('spinbutton', {
      name: '默认返回长度（字符）'
    });
    expect(budget).toHaveValue('1200');
    const fields = within(dialog).getByLabelText('默认返回字段白名单');
    expect(fields).toHaveValue('["/title"]');
    expect(fields.tagName).toBe('INPUT');
    fireEvent.change(budget, { target: { value: '80000' } });
    fireEvent.change(fields, { target: { value: '[]' } });
    fireEvent.click(within(dialog).getByText('OK'));
    await waitFor(() =>
      expect(api.updateSettingsMcpTool).toHaveBeenCalledWith(
        'read_tab',
        expect.objectContaining({
          max_inline_chars: 80000,
          response_fields: []
        }),
        expect.any(String)
      )
    );
  });

  it('keeps optional defaults blank and rejects malformed JSON Pointer fields', async () => {
    const dialog = await openEditor();
    const budget = within(dialog).getByRole('spinbutton', {
      name: '默认返回长度（字符）'
    });
    const fields = within(dialog).getByLabelText('默认返回字段白名单');
    fireEvent.change(budget, { target: { value: '' } });
    fireEvent.change(fields, { target: { value: '["title"]' } });
    fireEvent.click(within(dialog).getByText('OK'));
    expect(
      await within(dialog).findByText('请输入有效的 JSON Pointer 字符串数组。')
    ).toBeInTheDocument();
    expect(api.updateSettingsMcpTool).not.toHaveBeenCalled();
    fireEvent.change(fields, { target: { value: '' } });
    fireEvent.click(within(dialog).getByText('OK'));
    await waitFor(() =>
      expect(api.updateSettingsMcpTool).toHaveBeenCalledWith(
        'read_tab',
        expect.objectContaining({
          max_inline_chars: null,
          response_fields: null
        }),
        expect.any(String)
      )
    );
  });
});
