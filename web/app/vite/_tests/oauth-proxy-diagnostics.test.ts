import type { IncomingMessage } from 'node:http';

import { expect, test } from 'vitest';

import { oauthProxyDiagnostics } from '../oauth-proxy-diagnostics';

test('correlates discovery requests and responses without logging secrets', () => {
  const lines: string[] = [];
  const diagnostics = oauthProxyDiagnostics((line) => lines.push(line));
  const request = {
    method: 'GET',
    url: '/api/mcp/instance?api_key=secret-query',
    headers: {
      host: '127.0.0.1',
      'x-forwarded-host': 'example.com',
      'x-forwarded-proto': 'https',
      'user-agent': 'test-client',
      authorization: 'Bearer secret-token',
      cookie: 'secret-cookie'
    }
  } as IncomingMessage;
  diagnostics.request(request);
  diagnostics.response(request, 405);
  const records = lines.map((line) =>
    JSON.parse(line.slice('[mcp-oauth-proxy] '.length))
  );
  expect(records[0]).toMatchObject({
    method: 'GET',
    path: '/api/mcp/instance',
    forwarded_host: 'example.com',
    user_agent: 'test-client',
    has_authorization: true
  });
  expect(records[1]).toMatchObject({
    event: 'response',
    status: 405,
    request_id: records[0].request_id
  });
  expect(records[1].elapsed_ms).toBeGreaterThanOrEqual(0);
  expect(lines.join('')).not.toMatch(/secret-query|secret-token|secret-cookie/);
});

test('ignores unrelated requests and bounds hostile diagnostic fields', () => {
  const lines: string[] = [];
  const diagnostics = oauthProxyDiagnostics((line) => lines.push(line));
  diagnostics.request({
    url: '/api/console/users',
    headers: {}
  } as IncomingMessage);
  expect(lines).toHaveLength(0);
  diagnostics.error(
    {
      method: 'POST',
      url: '/api/public/mcp-oauth/token?code=secret-code',
      headers: { 'user-agent': 'x'.repeat(1000) + '\r\nsecret' }
    } as IncomingMessage,
    'ECONNREFUSED'
  );
  const record = JSON.parse(lines[0].slice('[mcp-oauth-proxy] '.length));
  expect(record).toMatchObject({
    event: 'error',
    error_code: 'ECONNREFUSED',
    path: '/api/public/mcp-oauth/token'
  });
  expect(record.user_agent).toHaveLength(256);
  expect(lines[0]).not.toContain('secret');
});
