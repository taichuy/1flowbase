import { createServer as createHttpServer, request } from 'node:http';
import type { AddressInfo } from 'node:net';

import { createServer } from 'vite';
import { expect, test } from 'vitest';

import { oauthAwareApiProxy } from '../oauth-proxy';

test('preserves public authority through an HTTPS tunnel and local HTTP', async () => {
  const upstream = createHttpServer((incoming, response) => {
    response.setHeader('content-type', 'application/json');
    response.end(
      JSON.stringify({
        host: incoming.headers.host,
        proto: incoming.headers['x-forwarded-proto'] ?? null
      })
    );
  });
  await new Promise<void>((resolve) =>
    upstream.listen(0, '127.0.0.1', resolve)
  );
  const upstreamPort = (upstream.address() as AddressInfo).port;
  let vite;
  let ingress;
  try {
    const proxy = oauthAwareApiProxy(`http://127.0.0.1:${upstreamPort}`);
    vite = await createServer({
      configFile: false,
      logLevel: 'silent',
      server: {
        middlewareMode: true,
        allowedHosts: ['tunnel.example.com'],
        hmr: false,
        proxy: {
          '/api': proxy,
          '/.well-known/oauth-': proxy,
          '/.well-known/openid-configuration': proxy
        }
      }
    });
    ingress = createHttpServer(vite.middlewares);
    await new Promise<void>((resolve) =>
      ingress!.listen(0, '127.0.0.1', resolve)
    );
    const port = (ingress.address() as AddressInfo).port;
    const cases = [
      {
        headers: { host: 'tunnel.example.com', 'x-forwarded-proto': 'https' },
        expected: { host: 'tunnel.example.com', proto: 'https' }
      },
      {
        headers: {
          host: '127.0.0.1',
          'x-forwarded-host': 'tunnel.example.com:8443',
          'x-forwarded-proto': 'https'
        },
        expected: { host: 'tunnel.example.com:8443', proto: 'https' }
      },
      {
        headers: { host: 'localhost:3100' },
        expected: { host: 'localhost:3100', proto: null }
      },
      // Preserve malformed headers so the API can reject them rather than silently choosing a value.
      {
        headers: {
          host: '127.0.0.1',
          'x-forwarded-host': 'one.example,two.example',
          'x-forwarded-proto': 'https,http'
        },
        expected: { host: 'one.example,two.example', proto: 'https,http' }
      }
    ];
    for (const route of [
      '/api/public/mcp-oauth/config',
      '/.well-known/oauth-authorization-server/api/public/mcp-oauth',
      '/.well-known/openid-configuration/api/public/mcp-oauth',
      '/api/public/mcp-oauth/.well-known/openid-configuration',
      '/api/public/mcp-oauth/protected-resource/1flowbase'
    ]) {
      for (const { headers, expected } of cases) {
        const actual = await new Promise((resolve, reject) => {
          const outgoing = request(
            { hostname: '127.0.0.1', port, path: route, headers },
            (response) => {
              let body = '';
              response.on('data', (chunk) => {
                body += chunk;
              });
              response.on('end', () => {
                try {
                  resolve(JSON.parse(body));
                } catch (error) {
                  reject(error);
                }
              });
            }
          );
          outgoing.on('error', reject);
          outgoing.end();
        });
        expect(actual).toEqual(expected);
      }
    }
  } finally {
    if (ingress)
      await new Promise<void>((resolve, reject) =>
        ingress!.close((error) => (error ? reject(error) : resolve()))
      );
    if (vite) await vite.close();
    await new Promise<void>((resolve, reject) =>
      upstream.close((error) => (error ? reject(error) : resolve()))
    );
  }
});
