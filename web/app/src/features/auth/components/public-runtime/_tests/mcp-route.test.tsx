import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { PublicRouterProvider } from '../../../../../app/public-router';

const api = vi.hoisted(() => ({
  fetchMcpOAuthAuthorization: vi.fn(),
  verifyMcpOAuthApiKey: vi.fn(),
  decideMcpOAuthAuthorization: vi.fn()
}));
vi.mock('../../../api/mcp-oauth', () => api);
beforeEach(() => {
  localStorage.clear();
  history.replaceState(
    {},
    '',
    '/mcp/authorize?request_id=req-one&origin=https%3A%2F%2Fclient.example&language=en'
  );
  Object.values(api).forEach((fn) => fn.mockReset());
  api.fetchMcpOAuthAuthorization.mockResolvedValue({
    client_name: 'Example client',
    instance_id: 'demo',
    scope: 'mcp:invoke'
  });
  api.verifyMcpOAuthApiKey.mockResolvedValue({
    approval_token: 'approval-token',
    workspace_name: 'Workspace',
    instance_name: 'Tools',
    scope: 'mcp:invoke'
  });
  api.decideMcpOAuthAuthorization.mockReturnValue(new Promise(() => undefined));
});
test('lazy MCP full providers retain request, origin, secret clearing and separate API-key consent', async () => {
  render(<PublicRouterProvider />);
  const key = await screen.findByLabelText('API key');
  expect(api.fetchMcpOAuthAuthorization).toHaveBeenCalledWith(
    'req-one',
    expect.any(AbortSignal),
    'https://client.example'
  );
  fireEvent.change(key, { target: { value: 'secret-canary' } });
  fireEvent.click(screen.getByRole('button', { name: 'Verify API key' }));
  expect(await screen.findByText('Workspace')).toBeVisible();
  expect(api.verifyMcpOAuthApiKey).toHaveBeenCalledWith(
    { request_id: 'req-one', api_key: 'secret-canary' },
    'https://client.example'
  );
  expect(api.decideMcpOAuthAuthorization).not.toHaveBeenCalled();
  expect(
    document.body.innerHTML + location.href + JSON.stringify(localStorage)
  ).not.toContain('secret-canary');
  fireEvent.click(screen.getByRole('button', { name: 'Authorize and return' }));
  await waitFor(() =>
    expect(api.decideMcpOAuthAuthorization).toHaveBeenCalledWith(
      {
        request_id: 'req-one',
        approval_token: 'approval-token',
        approved: true
      },
      'https://client.example'
    )
  );
});
test('changing request/origin through native history clears the old key and approval', async () => {
  render(<PublicRouterProvider />);
  fireEvent.change(await screen.findByLabelText('API key'), {
    target: { value: 'old-secret' }
  });
  await act(async () => {
    history.replaceState(
      {},
      '',
      '/mcp/authorize?request_id=req-two&origin=https%3A%2F%2Fother.example&language=en'
    );
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(await screen.findByLabelText('API key')).toHaveValue('');
  expect(api.fetchMcpOAuthAuthorization).toHaveBeenLastCalledWith(
    'req-two',
    expect.any(AbortSignal),
    'https://other.example'
  );
  expect(api.verifyMcpOAuthApiKey).not.toHaveBeenCalled();
  expect(
    screen.queryByRole('button', { name: 'Authorize and return' })
  ).not.toBeInTheDocument();
});
