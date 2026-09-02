import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import type { Plugin, ViteDevServer } from 'vite';

const HMR_PROBE_ID = 'virtual:1flowbase-dev-hmr-probe';
const RESOLVED_HMR_PROBE_ID = `\0${HMR_PROBE_ID}`;
const DEV_GENERATION_META_NAME = '1flowbase-dev-generation';
const DEV_CRITICAL_INTEROP_SPECIFIERS = ['is-mobile', 'react-is'] as const;
const DEV_GENERATIONS_RETAINED = 2;
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
  generation: string;
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
    path.resolve(root, '..', 'package.json'),
    path.resolve(root, 'package.json'),
    path.resolve(root, '.env'),
    path.resolve(root, '.env.local'),
    path.resolve(root, `.env.${mode}`),
    path.resolve(root, `.env.${mode}.local`),
    path.resolve(root, 'vite.config.ts'),
    path.resolve(root, 'vite', 'dev-runtime.ts')
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

function devGenerationCacheDirectory(root: string, mode: string) {
  return path.join(
    root,
    'node_modules',
    '.vite-generations',
    devCacheIdentity(root, mode)
  );
}

function verifyCriticalInteropCache(cacheDirectory: string) {
  const metadataPath = path.join(cacheDirectory, 'deps', '_metadata.json');
  if (!fs.existsSync(metadataPath)) {
    throw new Error(
      `optimized dependency metadata is missing: ${metadataPath}`
    );
  }
  const metadata = JSON.parse(fs.readFileSync(metadataPath, 'utf8')) as {
    optimized?: Record<string, unknown>;
  };
  const optimized = metadata.optimized || {};
  const missing = DEV_CRITICAL_INTEROP_SPECIFIERS.filter(
    (specifier) => !(specifier in optimized)
  );
  if (missing.length > 0) {
    throw new Error(
      `critical CommonJS dependencies were not optimized: ${missing.join(', ')}`
    );
  }
}

async function waitForCriticalInteropCache(
  cacheDirectory: string,
  timeoutMs = 30_000
) {
  const startedAt = Date.now();
  let latestError: unknown = null;
  while (Date.now() - startedAt < timeoutMs) {
    try {
      verifyCriticalInteropCache(cacheDirectory);
      return;
    } catch (error: unknown) {
      latestError = error;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw latestError instanceof Error
    ? latestError
    : new Error('critical CommonJS optimization timed out');
}

async function pruneDevGenerationCaches(
  root: string,
  activeGeneration: string
) {
  const generationsRoot = path.join(root, 'node_modules', '.vite-generations');
  if (!fs.existsSync(generationsRoot)) return [];

  const candidates = fs
    .readdirSync(generationsRoot, { withFileTypes: true })
    .filter(
      (entry) => entry.isDirectory() && /^[a-f0-9]{64}$/u.test(entry.name)
    )
    .map((entry) => {
      const directory = path.join(generationsRoot, entry.name);
      return {
        directory,
        generation: entry.name,
        modifiedAt: fs.statSync(directory).mtimeMs
      };
    })
    .sort((left, right) => right.modifiedAt - left.modifiedAt);

  const orderedGenerations = [
    activeGeneration,
    ...candidates.map((entry) => entry.generation)
  ]
    .filter(
      (generation, index, generations) =>
        generations.indexOf(generation) === index
    )
    .slice(0, DEV_GENERATIONS_RETAINED);
  const retained = new Set(orderedGenerations);
  const removed: string[] = [];
  for (const candidate of candidates) {
    if (retained.has(candidate.generation)) continue;
    await fs.promises.rm(candidate.directory, { recursive: true, force: true });
    removed.push(candidate.generation);
  }
  return removed;
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

async function warmRuntime(
  server: ViteDevServer,
  receipt: DevRuntimeReceipt,
  afterReady?: () => void
) {
  transitionRuntime(receipt, 'Warming');
  try {
    await Promise.all([
      server.transformRequest('/src/bootstrap.ts'),
      server.transformRequest('/src/main.tsx'),
      server.transformRequest('/src/app/App.tsx'),
      server.transformRequest('/src/app/ApplicationBootBoundary.tsx'),
      server.transformRequest('/src/app/ApplicationRuntimeBootstrap.tsx')
    ]);
    await waitForCriticalInteropCache(server.config.cacheDir);
    receipt.error = null;
    receipt.readyAt = new Date().toISOString();
    transitionRuntime(receipt, 'Ready');
    afterReady?.();
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
  receipt: DevRuntimeReceipt,
  afterReady?: () => void
) {
  server.middlewares.use(RECOVERY_PROBE_PATH, (_request, response) => {
    receipt.error = 'controlled recovery probe';
    transitionRuntime(receipt, 'Degraded');
    response.statusCode = 202;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(JSON.stringify({ state: receipt.state }));
    setImmediate(() => void warmRuntime(server, receipt, afterReady));
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
  const generation = devCacheIdentity(root, mode);
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
  const retireStaleGenerations = () => {
    if (serverCacheIsCustom()) return;
    setImmediate(() => {
      void pruneDevGenerationCaches(root, generation)
        .then((removed) => {
          if (removed.length > 0) {
            runtimeLogger?.info(
              `[1flowbase-dev-runtime] retired ${removed.length} stale dependency generation(s)`
            );
          }
        })
        .catch((error: unknown) => {
          runtimeLogger?.warn(
            `[1flowbase-dev-runtime] stale generation cleanup failed: ${
              error instanceof Error ? error.message : String(error)
            }`
          );
        });
    });
  };
  let runtimeLogger: ViteDevServer['config']['logger'] | null = null;
  const serverCacheIsCustom = () =>
    process.env.VITE_DEV_CACHE_DIR &&
    process.env.VITE_DEV_CACHE_DIR !== devGenerationCacheDirectory(root, mode);
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
    transformIndexHtml() {
      if (command !== 'serve') return [];
      return [
        {
          tag: 'meta',
          attrs: {
            name: DEV_GENERATION_META_NAME,
            content: generation
          },
          injectTo: 'head-prepend'
        }
      ];
    },
    configureServer(server) {
      runtimeLogger = server.config.logger;
      const receipt = {
        state: 'Scanning' as DevRuntimeState,
        cacheIdentity: generation,
        generation,
        startedAt: new Date().toISOString(),
        readyAt: null,
        error: null,
        transitions: [
          { state: 'Scanning' as DevRuntimeState, at: new Date().toISOString() }
        ]
      };
      attachReadinessMiddleware(server, receipt);
      if (hmrProbeFile) attachHmrProbeMiddleware(server, hmrProbeFile);
      attachRecoveryProbeMiddleware(server, receipt, retireStaleGenerations);
      attachPreReadyTrafficGate(server, receipt);
      transitionRuntime(receipt, 'Optimizing');
      server.httpServer?.once(
        'listening',
        () => void warmRuntime(server, receipt, retireStaleGenerations)
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
  DEV_CRITICAL_INTEROP_SPECIFIERS,
  DEV_GENERATION_META_NAME,
  HMR_PROBE_ID,
  HMR_PROBE_PATH,
  RECOVERY_PROBE_PATH,
  READY_PATH,
  devCacheIdentity,
  devGenerationCacheDirectory,
  hmrProbeSource,
  oneFlowbaseDevRuntimePlugin,
  pruneDevGenerationCaches,
  verifyCriticalInteropCache,
  waitForCriticalInteropCache
};
