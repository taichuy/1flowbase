import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
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

test('projects the same public authority into WebSocket upgrade requests', async () => {
  const cacheDir = await mkdtemp(join(tmpdir(), '1flowbase-ws-proxy-test-'));
  const received: Array<Record<string, string | string[] | undefined>> = [];
  const upstream = createHttpServer();
  upstream.on('upgrade', (incoming, socket) => {
    received.push({
      host: incoming.headers.host,
      origin: incoming.headers.origin,
      proto: incoming.headers['x-forwarded-proto']
    });
    // Mirror the backend's strict Origin/Host boundary, including ambiguous authorities.
    const matches =
      incoming.headers.origin &&
      new URL(incoming.headers.origin).host === incoming.headers.host;
    socket.end(
      matches
        ? 'HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n'
        : 'HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n'
    );
  });
  await new Promise<void>((resolve) =>
    upstream.listen(0, '127.0.0.1', resolve)
  );
  let vite;
  try {
    const port = (upstream.address() as AddressInfo).port;
    vite = await createServer({
      configFile: false,
      cacheDir,
      logLevel: 'silent',
      appType: 'custom',
      server: {
        host: '127.0.0.1',
        port: 0,
        hmr: false,
        allowedHosts: ['tunnel.example.com'],
        proxy: {
          '/api': {
            ...oauthAwareApiProxy(`http://127.0.0.1:${port}`),
            ws: true
          }
        }
      }
    });
    await vite.listen();
    const ingressPort = (vite.httpServer!.address() as AddressInfo).port;
    const cases = [
      {
        headers: {
          host: 'tunnel.example.com',
          origin: 'https://tunnel.example.com',
          'x-forwarded-proto': 'https'
        },
        status: 101,
        host: 'tunnel.example.com',
        proto: 'https'
      },
      {
        headers: {
          host: '127.0.0.1:3100',
          origin: 'https://tunnel.example.com:8443',
          'x-forwarded-host': 'tunnel.example.com:8443',
          'x-forwarded-proto': 'https'
        },
        status: 101,
        host: 'tunnel.example.com:8443',
        proto: 'https'
      },
      {
        headers: { host: 'localhost:3100', origin: 'http://localhost:3100' },
        status: 101,
        host: 'localhost:3100',
        proto: undefined
      },
      {
        headers: {
          host: '127.0.0.1:3100',
          origin: 'https://tunnel.example.com',
          'x-forwarded-host': 'one.example,two.example',
          'x-forwarded-proto': 'https,http'
        },
        status: 403,
        host: 'one.example,two.example',
        proto: 'https,http'
      }
    ];
    for (const { headers, status, host, proto } of cases) {
      const actual = await new Promise<number>((resolve, reject) => {
        const outgoing = request(
          {
            hostname: '127.0.0.1',
            port: ingressPort,
            path: '/api/console/managed-services/fixture/terminal/stream?ticket=fixture',
            headers: {
              ...headers,
              connection: 'Upgrade',
              upgrade: 'websocket',
              'sec-websocket-version': '13',
              'sec-websocket-key': 'dGhlIHNhbXBsZSBub25jZQ=='
            }
          },
          (response) => {
            response.resume();
            response.on('end', () => resolve(response.statusCode!));
          }
        );
        outgoing.on('upgrade', (response, socket) => {
          socket.destroy();
          resolve(response.statusCode!);
        });
        outgoing.on('error', reject);
        outgoing.end();
      });
      expect(actual, JSON.stringify(headers)).toBe(status);
      expect(received.at(-1)).toEqual({ host, origin: headers.origin, proto });
    }
  } finally {
    if (vite) await vite.close();
    await rm(cacheDir, { recursive: true, force: true });
    await new Promise<void>((resolve, reject) =>
      upstream.close((error) => (error ? reject(error) : resolve()))
    );
  }
});

test('uses target origin for HTTPS upstreams without overwriting Host from x-forwarded-host', () => {
  const proxy = oauthAwareApiProxy('https://remote.example.test');
  expect(proxy.changeOrigin).toBe(true);
  expect(proxy.xfwd).toBe(false);

  const handlers = new Map<string, (...args: unknown[]) => void>();
  proxy.configure?.(
    {
      on(event: string, handler: (...args: unknown[]) => void) {
        handlers.set(event, handler);
      }
    } as never,
    proxy
  );

  for (const event of ['proxyReq', 'proxyReqWs']) {
    let rewrittenHost: string | null = null;
    handlers.get(event)?.(
      {
        setHeader(name: string, value: string) {
          if (name.toLowerCase() === 'host') rewrittenHost = value;
        }
      },
      {
        method: 'GET',
        url: '/api/console/session',
        headers: {
          host: 'localhost:3100',
          'x-forwarded-host': 'tunnel.example.com'
        }
      }
    );
    expect(rewrittenHost).toBeNull();
  }
});
