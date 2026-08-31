import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import type { Plugin, ViteDevServer } from 'vite';

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
  mode
}: {
  root: string;
  mode: string;
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
      if (id === HMR_PROBE_ID) return RESOLVED_HMR_PROBE_ID;
      return null;
    },
    load(id) {
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
  HMR_PROBE_ID,
  HMR_PROBE_PATH,
  RECOVERY_PROBE_PATH,
  READY_PATH,
  devCacheIdentity,
  hmrProbeSource,
  oneFlowbaseDevRuntimePlugin
};
