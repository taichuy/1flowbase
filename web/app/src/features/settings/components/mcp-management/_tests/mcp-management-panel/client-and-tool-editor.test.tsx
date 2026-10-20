import { describe, test, expect } from 'vitest';
import {
  domEvents,
  mcpManagementApi,
  renderPanelWithMountedTool,
  screen,
  vditorMock,
  waitFor,
  within
} from './client-and-tool-editor.fixtures';

describe('McpManagementPanel', () => {
  test('AC-003 AC-004 renders persistent Agent CLI configuration commands with the current endpoint and API key', async () => {
    await renderPanelWithMountedTool();
    await domEvents.click(screen.getByRole('button', { name: '连接客户端' }));
    await domEvents.change(await screen.findByLabelText('API Key'), {
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

    await domEvents.click(within(dialog).getByRole('tab', { name: 'Codex' }));
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
    expect(codexRemovePreview).toHaveTextContent("codex mcp remove 'ops_mcp'");
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

    await domEvents.click(
      within(dialog).getByRole('tab', { name: 'Claude Code' })
    );
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

    await domEvents.click(
      within(dialog).getByRole('tab', { name: 'OpenCode' })
    );
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
    await renderPanelWithMountedTool();
    await domEvents.click(screen.getByRole('button', { name: '连接客户端' }));

    const dialog = await screen.findByRole('dialog');
    await domEvents.click(within(dialog).getByRole('tab', { name: 'Codex' }));

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

    await domEvents.change(within(dialog).getByLabelText('API Key'), {
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
    await renderPanelWithMountedTool();

    await domEvents.click(screen.getByRole('button', { name: '连接客户端' }));

    await waitFor(() => {
      expect(screen.getByLabelText('API Key')).toHaveValue('saved-secret-key');
    });
    expect(screen.queryByText('已加密保存')).not.toBeInTheDocument();
    await domEvents.click(screen.getByRole('button', { name: /清除认证/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.deleteSettingsMcpClientCredential
      ).toHaveBeenCalledWith('ops_mcp', expect.any(String));
    });
    expect(screen.getByLabelText('API Key')).toHaveValue('');
  });

  test('saves the MCP client API key from the modal footer without a switch', async () => {
    await renderPanelWithMountedTool();
    await domEvents.click(screen.getByRole('button', { name: '连接客户端' }));
    await domEvents.change(await screen.findByLabelText('API Key'), {
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
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      clearButton.compareDocumentPosition(saveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(saveButton).toHaveClass('ant-btn-primary');
    await domEvents.click(screen.getByRole('button', { name: /保存$/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.saveSettingsMcpClientCredential
      ).toHaveBeenCalledWith('ops_mcp', 'new-secret-key', expect.any(String));
    });
    expect(screen.queryByText('已加密保存')).not.toBeInTheDocument();
  });
});
