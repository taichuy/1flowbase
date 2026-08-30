import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import type { Plugin, ViteDevServer } from 'vite';

const ICON_REGISTRY_ID = 'virtual:1flowbase-page-tree-icons';
const RESOLVED_ICON_REGISTRY_ID = `\0${ICON_REGISTRY_ID}`;
const HMR_PROBE_ID = 'virtual:1flowbase-dev-hmr-probe';
const RESOLVED_HMR_PROBE_ID = `\0${HMR_PROBE_ID}`;
const READY_PATH = '/__1flowbase_dev_ready';
const HMR_PROBE_PATH = '/__1flowbase_dev_hmr_probe';

type DevRuntimeState =
  | 'Scanning'
  | 'Optimizing'
  | 'Warming'
  | 'Ready'
  | 'Degraded';

function pageTreeIconNames(root: string) {
  const iconsDirectory = path.join(
    root,
    'node_modules',
    '@ant-design',
    'icons',
    'es',
    'icons'
  );

  return fs
    .readdirSync(iconsDirectory)
    .filter((name) => /(?:Outlined|Filled|TwoTone)\.js$/u.test(name))
    .map((name) => name.replace(/\.js$/u, ''))
    .sort((left, right) => left.localeCompare(right));
}

function iconRegistrySource(root: string, command: 'serve' | 'build') {
  const iconNames = pageTreeIconNames(root);
  if (command === 'build') {
    return [
      `import * as AntIcons from '@ant-design/icons';`,
      `export const pageTreeIconNames = ${JSON.stringify(iconNames)};`,
      `export const pageTreeIconLoaders = Object.fromEntries(pageTreeIconNames.map((name) => [name, () => Promise.resolve(AntIcons[name])]));`
    ].join('\n');
  }
  const loaders = iconNames
    .map(
      (iconName) =>
        `${JSON.stringify(iconName)}: () => import(${JSON.stringify(
          `@ant-design/icons/${iconName}`
        )}).then((module) => module.default)`
    )
    .join(',\n');

  return [
    `export const pageTreeIconNames = ${JSON.stringify(iconNames)};`,
    `export const pageTreeIconLoaders = {${loaders}};`
  ].join('\n');
}

function devCacheIdentity(root: string, mode: string) {
  const hash = crypto.createHash('sha256');
  const inputs = [
    path.resolve(root, '..', 'pnpm-lock.yaml'),
    path.resolve(root, 'vite.config.ts')
  ];

  for (const filePath of inputs) {
    hash.update(filePath);
    hash.update('\0');
    hash.update(
      fs.existsSync(filePath) ? fs.readFileSync(filePath) : '<missing>'
    );
    hash.update('\0');
  }

  const viteEnvironment = Object.fromEntries(
    Object.entries(process.env)
      .filter(([name]) => name.startsWith('VITE_'))
      .sort(([left], [right]) => left.localeCompare(right))
  );
  hash.update(
    JSON.stringify({
      runtime: process.version,
      platform: process.platform,
      arch: process.arch,
      mode,
      viteEnvironment
    })
  );
  return hash.digest('hex');
}

function attachReadinessMiddleware(
  server: ViteDevServer,
  receipt: {
    state: DevRuntimeState;
    cacheIdentity: string;
    startedAt: string;
    readyAt: string | null;
    error: string | null;
  }
) {
  server.middlewares.use(READY_PATH, (_request, response) => {
    response.statusCode = receipt.state === 'Ready' ? 200 : 503;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(
      JSON.stringify({
        schema_version: '1flowbase.dev-runtime-readiness/v1',
        ...receipt
      })
    );
  });
}

function attachHmrProbeMiddleware(server: ViteDevServer) {
  server.middlewares.use(HMR_PROBE_PATH, (_request, response) => {
    const token = crypto.randomUUID();
    const sentAt = Date.now();
    server.ws.send({
      type: 'custom',
      event: '1flowbase:dev-hmr-probe',
      data: { token, sentAt }
    });
    response.statusCode = 200;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(JSON.stringify({ token, sentAt }));
  });
}

function hmrProbeSource() {
  return `
    if (import.meta.hot) {
      import.meta.hot.on('1flowbase:dev-hmr-probe', (receipt) => {
        globalThis.__ONEFLOWBASE_DEV_HMR_RECEIPT__ = {
          ...receipt,
          receivedAt: Date.now()
        };
      });
    }
    export {};
  `;
}

function warmRuntime(
  server: ViteDevServer,
  receipt: {
    state: DevRuntimeState;
    readyAt: string | null;
    error: string | null;
  }
) {
  receipt.state = 'Warming';
  Promise.all([
    server.transformRequest('/src/main.tsx'),
    server.transformRequest('/src/app/router.tsx')
  ])
    .then(() => {
      receipt.state = 'Ready';
      receipt.readyAt = new Date().toISOString();
    })
    .catch((error: unknown) => {
      receipt.state = 'Degraded';
      receipt.error = error instanceof Error ? error.message : String(error);
      server.config.logger.error(
        `[1flowbase-dev-runtime] warmup failed: ${receipt.error}`
      );
    });
}

function oneFlowbaseDevRuntimePlugin({
  root,
  mode,
  command
}: {
  root: string;
  mode: string;
  command: 'serve' | 'build';
}): Plugin {
  return {
    name: '1flowbase-dev-runtime',
    enforce: 'pre',
    resolveId(id) {
      if (id === ICON_REGISTRY_ID) return RESOLVED_ICON_REGISTRY_ID;
      if (id === HMR_PROBE_ID) return RESOLVED_HMR_PROBE_ID;
      return null;
    },
    load(id) {
      if (id === RESOLVED_ICON_REGISTRY_ID)
        return iconRegistrySource(root, command);
      if (id === RESOLVED_HMR_PROBE_ID) return hmrProbeSource();
      return null;
    },
    configureServer(server) {
      const receipt = {
        state: 'Scanning' as DevRuntimeState,
        cacheIdentity: devCacheIdentity(root, mode),
        startedAt: new Date().toISOString(),
        readyAt: null,
        error: null
      };
      attachReadinessMiddleware(server, receipt);
      attachHmrProbeMiddleware(server);
      receipt.state = 'Optimizing';
      server.httpServer?.once('listening', () => warmRuntime(server, receipt));
    }
  };
}

export {
  ICON_REGISTRY_ID,
  HMR_PROBE_ID,
  HMR_PROBE_PATH,
  READY_PATH,
  devCacheIdentity,
  iconRegistrySource,
  hmrProbeSource,
  oneFlowbaseDevRuntimePlugin,
  pageTreeIconNames
};
