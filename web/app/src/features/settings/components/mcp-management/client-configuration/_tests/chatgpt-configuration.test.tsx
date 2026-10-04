import { render, screen, cleanup } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { ChatGptConfiguration } from '../ChatGptConfiguration';

const fetchConfig = vi.hoisted(() => vi.fn());
vi.mock('../../../../api/mcp-oauth', () => ({
  fetchMcpOAuthConfiguration: fetchConfig
}));
const show = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ChatGptConfiguration instanceId="demo" />
    </QueryClientProvider>
  );
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

test('shows trusted backend URL and DCR instructions without a Key input', async () => {
  fetchConfig.mockResolvedValue({
    enabled: true,
    server_url: 'https://trusted.example/api/mcp/demo',
    scope: 'mcp:invoke',
    registration_method: 'dynamic_client_registration'
  });
  show();
  expect(
    await screen.findByText('https://trusted.example/api/mcp/demo')
  ).toBeInTheDocument();
  expect(screen.getByText('动态客户端注册（DCR）')).toBeInTheDocument();
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});

test('disabled deployment cannot present a guessed usable URL', async () => {
  fetchConfig.mockResolvedValue({
    enabled: false,
    server_url: null,
    scope: 'mcp:invoke',
    registration_method: 'dynamic_client_registration'
  });
  show();
  expect(
    await screen.findByText('此部署尚未启用 ChatGPT 授权')
  ).toBeInTheDocument();
  expect(screen.queryByText('服务器 URL')).not.toBeInTheDocument();
});

test('configuration failures stay distinct from disabled configuration', async () => {
  fetchConfig.mockRejectedValue(new Error('unavailable'));
  show();
  expect(
    await screen.findByText('无法读取 ChatGPT 连接配置。')
  ).toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: '重新读取配置' })
  ).toBeInTheDocument();
  expect(
    screen.queryByText('此部署尚未启用 ChatGPT 授权')
  ).not.toBeInTheDocument();
});
