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
const RECOVERY_PROBE_PATH = '/__1flowbase_dev_recovery_probe';

type DevRuntimeState =
  | 'Scanning'
  | 'Optimizing'
  | 'Warming'
  | 'Ready'
  | 'Degraded';

type DevRuntimeReceipt = {
  state: DevRuntimeState;
  cacheIdentity: string;
  startedAt: string;
  readyAt: string | null;
  error: string | null;
  transitions: Array<{ state: DevRuntimeState; at: string }>;
};

function transitionRuntime(receipt: DevRuntimeReceipt, state: DevRuntimeState) {
  receipt.state = state;
  receipt.transitions.push({ state, at: new Date().toISOString() });
}

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
  receipt: DevRuntimeReceipt
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

function attachHmrProbeMiddleware(server: ViteDevServer, probeFile: string) {
  server.middlewares.use(HMR_PROBE_PATH, (_request, response) => {
    const token = crypto.randomUUID();
    const sentAt = Date.now();
    fs.writeFileSync(
      probeFile,
      `export const generation = ${JSON.stringify(token)};\n`,
      'utf8'
    );
    response.statusCode = 200;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(JSON.stringify({ token, sentAt }));
  });
}

function hmrProbeSource(probeFile: string | null) {
  if (!probeFile) return 'export {};';
  const probeImport = `/@fs/${probeFile.replace(/\\/gu, '/')}`;
  return `
    import { generation } from ${JSON.stringify(probeImport)};
    globalThis.__ONEFLOWBASE_DEV_HMR_GENERATION__ = generation;
    if (import.meta.hot) {
      import.meta.hot.accept(${JSON.stringify(probeImport)}, (module) => {
        globalThis.__ONEFLOWBASE_DEV_HMR_RECEIPT__ = {
          token: module.generation,
          receivedAt: Date.now()
        };
      });
    }
    export {};
  `;
}

async function warmRuntime(server: ViteDevServer, receipt: DevRuntimeReceipt) {
  transitionRuntime(receipt, 'Warming');
  try {
    await Promise.all([
      server.transformRequest('/src/main.tsx'),
      server.transformRequest('/src/app/router.tsx')
    ]);
    receipt.error = null;
    receipt.readyAt = new Date().toISOString();
    transitionRuntime(receipt, 'Ready');
  } catch (error: unknown) {
    receipt.error = error instanceof Error ? error.message : String(error);
    transitionRuntime(receipt, 'Degraded');
    server.config.logger.error(
      `[1flowbase-dev-runtime] warmup failed: ${receipt.error}`
    );
  }
}

function attachRecoveryProbeMiddleware(
  server: ViteDevServer,
  receipt: DevRuntimeReceipt
) {
  server.middlewares.use(RECOVERY_PROBE_PATH, (_request, response) => {
    receipt.error = 'controlled recovery probe';
    transitionRuntime(receipt, 'Degraded');
    response.statusCode = 202;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(JSON.stringify({ state: receipt.state }));
    setImmediate(() => void warmRuntime(server, receipt));
  });
}

function attachPreReadyTrafficGate(
  server: ViteDevServer,
  receipt: DevRuntimeReceipt
) {
  server.middlewares.use((_request, response, next) => {
    if (receipt.state === 'Ready') {
      next();
      return;
    }
    response.statusCode = 503;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(
      JSON.stringify({
        schema_version: '1flowbase.dev-runtime-readiness/v1',
        state: receipt.state
      })
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
  const runtimeDirectory = path.join(
    path.resolve(root, '..', '..'),
    'tmp',
    'dev-runtime'
  );
  const hmrProbeFile =
    command === 'serve'
      ? path.join(
          runtimeDirectory,
          `hmr-probe-${process.pid}-${crypto.randomUUID()}.mjs`
        )
      : null;
  if (hmrProbeFile) {
    fs.mkdirSync(runtimeDirectory, { recursive: true });
    fs.writeFileSync(hmrProbeFile, 'export const generation = "boot";\n');
  }
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
      if (id === RESOLVED_HMR_PROBE_ID) return hmrProbeSource(hmrProbeFile);
      return null;
    },
    configureServer(server) {
      const receipt = {
        state: 'Scanning' as DevRuntimeState,
        cacheIdentity: devCacheIdentity(root, mode),
        startedAt: new Date().toISOString(),
        readyAt: null,
        error: null,
        transitions: [
          { state: 'Scanning' as DevRuntimeState, at: new Date().toISOString() }
        ]
      };
      attachReadinessMiddleware(server, receipt);
      if (hmrProbeFile) attachHmrProbeMiddleware(server, hmrProbeFile);
      attachRecoveryProbeMiddleware(server, receipt);
      attachPreReadyTrafficGate(server, receipt);
      transitionRuntime(receipt, 'Optimizing');
      server.httpServer?.once(
        'listening',
        () => void warmRuntime(server, receipt)
      );
      server.httpServer?.once('close', () => {
        if (hmrProbeFile && fs.existsSync(hmrProbeFile)) {
          fs.unlinkSync(hmrProbeFile);
        }
      });
    }
  };
}

export {
  ICON_REGISTRY_ID,
  HMR_PROBE_ID,
  HMR_PROBE_PATH,
  RECOVERY_PROBE_PATH,
  READY_PATH,
  devCacheIdentity,
  iconRegistrySource,
  hmrProbeSource,
  oneFlowbaseDevRuntimePlugin,
  pageTreeIconNames
};
