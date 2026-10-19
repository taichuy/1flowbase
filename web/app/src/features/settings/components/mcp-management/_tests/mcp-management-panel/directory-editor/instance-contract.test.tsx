import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import {
  setupDirectoryEditorFixture,
  click,
  change,
  renderPanel,
  renderPanelWithMountedTool,
  mcpManagementApi
} from './fixture';

describe('McpManagementPanel', () => {
  setupDirectoryEditorFixture();

  test('generates a random instance_id from the instance modal action', async () => {
    const randomSpy = vi.spyOn(Math, 'random').mockReturnValue(0.123456789);

    try {
      await renderPanel();

      await click(screen.getByRole('tab', { name: 'MCP 实例' }));
      await click(screen.getByRole('button', { name: /新增/ }));

      const dialog = await screen.findByRole('dialog');
      const instanceIdField = within(dialog).getByLabelText('instance_id');

      expect(instanceIdField).toHaveValue('');

      await click(
        within(dialog).getByRole('button', { name: '随机生成 instance_id' })
      );

      expect(instanceIdField).toHaveValue('04fzzzxj');
    } finally {
      randomSpy.mockRestore();
    }
  });

  test('opens the requested tab from the URL search param', async () => {
    window.history.replaceState({}, '', '/settings/mcp-management?tab=tools');

    await renderPanel();

    expect(screen.getByRole('tab', { name: 'Tool 配置' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });

  test('AC-001 restores the third-party MCP tab from the URL', async () => {
    window.history.replaceState(
      {},
      '',
      '/settings/mcp-management?tab=third-party'
    );

    await renderPanel();

    expect(screen.getAllByRole('tab')).toHaveLength(3);
    expect(screen.getByRole('tab', { name: '第三方MCP' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });

  test('falls back from the removed meta tab to instances', async () => {
    window.history.replaceState({}, '', '/settings/mcp-management?tab=meta');
    await renderPanel();

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
    await renderPanelWithMountedTool();

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    await click(
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
    await click(
      within(dialog).getByRole('button', { name: /添加返回字段/ })
    );
    await change(within(dialog).getByLabelText('列表返回字段 1'), {
      target: { value: 'id' }
    });
    expect(
      within(dialog).queryByText('包含参数映射摘要')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('描述版本校验')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('参数错误格式')).not.toBeInTheDocument();

    await change(within(dialog).getByLabelText('默认返回数量'), {
      target: { value: '30' }
    });
    await change(within(dialog).getByLabelText('最大目录深度'), {
      target: { value: '4' }
    });
    await click(within(dialog).getByLabelText('允许正则路径查询'));
    await change(within(dialog).getByLabelText('正则表达式最大长度'), {
      target: { value: '160' }
    });
    await click(within(dialog).getByRole('tab', { name: 'JSON 解析' }));
    await change(
      await within(dialog).findByLabelText('列表返回字段 JSON'),
      {
        target: { value: '["id","name"]' }
      }
    );
    await click(within(dialog).getByRole('button', { name: /保存/ }));

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
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
  });

  test('AC-001 AC-002 fills a temporary API key into the common MCP JSON tab', async () => {
    await renderPanelWithMountedTool();

    await click(screen.getByRole('button', { name: '连接客户端' }));

    expect(await screen.findByText('MCP 客户端配置')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toHaveTextContent(
      `${window.location.origin}/api/mcp/ops_mcp`
    );
    await change(screen.getByLabelText('API Key'), {
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
    ).toEqual(['通用', 'ChatGPT', 'Codex', 'Claude Code', 'OpenCode']);

    await click(screen.getByRole('button', { name: '关 闭' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    await click(screen.getByRole('button', { name: '连接客户端' }));
    expect(await screen.findByLabelText('API Key')).toHaveValue('');
  });
});
