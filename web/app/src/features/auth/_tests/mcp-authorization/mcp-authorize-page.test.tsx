import {
  fireEvent,
  render,
  screen,
  waitFor,
  cleanup
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { PublicAuthProviders } from '../../components/PublicAuthProviders';
import { McpAuthorizePage } from '../../pages/mcp-authorization/McpAuthorizePage';

const api = vi.hoisted(() => ({
  fetchMcpOAuthAuthorization: vi.fn(),
  verifyMcpOAuthApiKey: vi.fn(),
  decideMcpOAuthAuthorization: vi.fn()
}));
vi.mock('../../api/mcp-oauth', () => api);
const renderPage = (requestId?: string) =>
  render(
    <PublicAuthProviders>
      <McpAuthorizePage requestId={requestId} />
    </PublicAuthProviders>
  );
const request = {
  client_name: 'ChatGPT',
  instance_id: 'demo',
  scope: 'mcp:invoke'
};
const approval = {
  approval_token: 'approval-secret',
  workspace_name: '研发工作区',
  instance_name: '演示工具',
  scope: 'mcp:invoke'
};

beforeEach(() => {
  vi.resetAllMocks();
  api.fetchMcpOAuthAuthorization.mockResolvedValue(request);
  api.verifyMcpOAuthApiKey.mockResolvedValue(approval);
  api.decideMcpOAuthAuthorization.mockImplementation(
    () => new Promise(() => {})
  );
});
afterEach(cleanup);

describe('API Key MCP authorization without a console session', () => {
  test('verifies, clears Key, shows server-owned workspace, and requires a separate consent', async () => {
    renderPage('req');
    const key = await screen.findByLabelText('API Key');
    expect(
      screen.queryByRole('button', { name: '同意授权并返回' })
    ).not.toBeInTheDocument();
    fireEvent.change(key, { target: { value: 'pat_secret_canary' } });
    fireEvent.click(screen.getByRole('button', { name: '验证 API Key' }));
    expect(await screen.findByText('研发工作区')).toBeInTheDocument();
    expect(screen.getByText('演示工具')).toBeInTheDocument();
    expect(api.verifyMcpOAuthApiKey).toHaveBeenCalledWith({
      request_id: 'req',
      api_key: 'pat_secret_canary'
    });
    expect(api.decideMcpOAuthAuthorization).not.toHaveBeenCalled();
    expect(document.body.innerHTML).not.toContain('pat_secret_canary');
    expect(window.location.href).not.toContain('pat_secret_canary');
    expect(JSON.stringify(localStorage)).not.toContain('pat_secret_canary');
    fireEvent.click(screen.getByRole('button', { name: '同意授权并返回' }));
    await waitFor(() =>
      expect(api.decideMcpOAuthAuthorization).toHaveBeenCalledWith({
        request_id: 'req',
        approval_token: 'approval-secret',
        approved: true
      })
    );
  });

  test('denies without asking for an API Key or minting approval', async () => {
    renderPage('req');
    fireEvent.click(await screen.findByRole('button', { name: '拒绝授权' }));
    await waitFor(() =>
      expect(api.decideMcpOAuthAuthorization).toHaveBeenCalledWith({
        request_id: 'req',
        approved: false
      })
    );
    expect(api.verifyMcpOAuthApiKey).not.toHaveBeenCalled();
  });

  test('rejects invalid requests and never shows an actionable form', async () => {
    api.fetchMcpOAuthAuthorization.mockRejectedValue(new Error('expired'));
    renderPage('expired');
    expect(await screen.findByRole('alert')).toHaveTextContent(
      '授权请求无效或已过期'
    );
    expect(screen.queryByLabelText('API Key')).not.toBeInTheDocument();
  });

  test('requires a request and cannot turn a standalone page into a login', () => {
    renderPage();
    expect(screen.getByRole('alert')).toHaveTextContent('授权请求无效或已过期');
    expect(api.fetchMcpOAuthAuthorization).not.toHaveBeenCalled();
  });

  test('a failed Key verification clears the secret and does not grant consent', async () => {
    api.verifyMcpOAuthApiKey.mockRejectedValue(
      new Error('internal detail must not reach UI')
    );
    renderPage('req');
    fireEvent.change(await screen.findByLabelText('API Key'), {
      target: { value: 'pat_wrong' }
    });
    fireEvent.click(screen.getByRole('button', { name: '验证 API Key' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      '无法验证 API Key'
    );
    expect(screen.getByLabelText('API Key')).toHaveValue('');
    expect(
      screen.queryByRole('button', { name: '同意授权并返回' })
    ).not.toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain('internal detail');
  });

  test('changing the Key discards the previous approval and requires verification again', async () => {
    renderPage('req');
    fireEvent.change(await screen.findByLabelText('API Key'), {
      target: { value: 'pat_old' }
    });
    fireEvent.click(screen.getByRole('button', { name: '验证 API Key' }));
    fireEvent.click(
      await screen.findByRole('button', { name: '使用其他 API Key' })
    );
    expect(screen.getByLabelText('API Key')).toHaveValue('');
    expect(screen.queryByText('研发工作区')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '同意授权并返回' })
    ).not.toBeInTheDocument();
  });
});
