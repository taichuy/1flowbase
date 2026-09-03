import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import type { Plugin, ViteDevServer } from 'vite';

const HMR_PROBE_ID = 'virtual:1flowbase-dev-hmr-probe';
const RESOLVED_HMR_PROBE_ID = `\0${HMR_PROBE_ID}`;
const DEV_GENERATION_META_NAME = '1flowbase-dev-generation';
const DEV_GENERATION_MANIFEST_NAME = '1flowbase-generation-manifest.json';
const DEV_CRITICAL_INTEROP_SPECIFIERS = ['is-mobile', 'react-is'] as const;
const DEV_GENERATIONS_RETAINED = 2;
const DEV_WORKSPACE_DEPENDENCY_ROOTS = ['packages/api-client'] as const;
const READY_PATH = '/__1flowbase_dev_ready';
const HMR_PROBE_PATH = '/__1flowbase_dev_hmr_probe';
const RECOVERY_PROBE_PATH = '/__1flowbase_dev_recovery_probe';

type DevRuntimeState =
  | 'Building'
  | 'Validating'
  | 'Ready'
  | 'Retired'
  | 'Degraded';

type DevRuntimeStage =
  | 'entry_transform'
  | 'optimizer_contract'
  | 'manifest_publish'
  | 'recovery';

type DevRuntimeError = {
  stage: DevRuntimeStage;
  specifier?: string;
  name: string;
  message: string;
};

type DevGenerationInput = {
  path: string;
  kind: 'config' | 'workspace-manifest' | 'workspace-source';
  digest: string;
};

type DevGenerationDependencyManifest = {
  schemaVersion: '1flowbase.dev-generation-manifest/v1';
  mode: string;
  toolchain: {
    runtime: string;
    platform: NodeJS.Platform;
    arch: string;
  };
  viteEnvironment: Record<string, string>;
  inputs: DevGenerationInput[];
};

type DevRuntimeReceipt = {
  state: DevRuntimeState;
  cacheIdentity: string;
  generation: string;
  startedAt: string;
  readyAt: string | null;
  error: DevRuntimeError | null;
  transitions: Array<{ state: DevRuntimeState; at: string }>;
};

function transitionRuntime(receipt: DevRuntimeReceipt, state: DevRuntimeState) {
  receipt.state = state;
  receipt.transitions.push({ state, at: new Date().toISOString() });
}

function sha256(content: string | Buffer) {
  return crypto.createHash('sha256').update(content).digest('hex');
}

function normalizeManifestPath(webRoot: string, filePath: string) {
  return path.relative(webRoot, filePath).split(path.sep).join('/');
}

function collectRuntimeSourceFiles(directory: string): string[] {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .flatMap((entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === '_tests' || entry.name === 'node_modules') return [];
        return collectRuntimeSourceFiles(entryPath);
      }
      if (!entry.isFile()) return [];
      if (/\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(entry.name)) return [];
      return /\.[cm]?[jt]sx?$/u.test(entry.name) ? [entryPath] : [];
    });
}

function createDevGenerationDependencyManifest(
  root: string,
  mode: string
): DevGenerationDependencyManifest {
  const webRoot = path.resolve(root, '..');
  const configInputs = [
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
  const viteEnvironment = Object.fromEntries(
    Object.entries(process.env)
      .filter(([name]) => name.startsWith('VITE_'))
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, value]) => [name, value || ''])
  );
  const inputs: DevGenerationInput[] = configInputs.map((filePath) => ({
    path: normalizeManifestPath(webRoot, filePath),
    kind: 'config',
    digest: sha256(
      fs.existsSync(filePath) ? fs.readFileSync(filePath) : '<missing>'
    )
  }));
  for (const dependencyRoot of DEV_WORKSPACE_DEPENDENCY_ROOTS) {
    const absoluteRoot = path.join(webRoot, dependencyRoot);
    const packageManifest = path.join(absoluteRoot, 'package.json');
    inputs.push({
      path: normalizeManifestPath(webRoot, packageManifest),
      kind: 'workspace-manifest',
      digest: sha256(
        fs.existsSync(packageManifest)
          ? fs.readFileSync(packageManifest)
          : '<missing>'
      )
    });
    for (const sourceFile of collectRuntimeSourceFiles(
      path.join(absoluteRoot, 'src')
    )) {
      inputs.push({
        path: normalizeManifestPath(webRoot, sourceFile),
        kind: 'workspace-source',
        digest: sha256(fs.readFileSync(sourceFile))
      });
    }
  }
  inputs.sort((left, right) => left.path.localeCompare(right.path));
  return {
    schemaVersion: '1flowbase.dev-generation-manifest/v1',
    mode,
    toolchain: {
      runtime: process.version,
      platform: process.platform,
      arch: process.arch
    },
    viteEnvironment,
    inputs
  };
}

function devCacheIdentity(root: string, mode: string) {
  return sha256(
    JSON.stringify(createDevGenerationDependencyManifest(root, mode))
  );
}

function persistDevGenerationDependencyManifest(
  cacheDirectory: string,
  manifest: DevGenerationDependencyManifest
) {
  fs.mkdirSync(cacheDirectory, { recursive: true });
  const manifestPath = path.join(cacheDirectory, DEV_GENERATION_MANIFEST_NAME);
  const stagingPath = `${manifestPath}.${process.pid}.${crypto.randomUUID()}.tmp`;
  fs.writeFileSync(
    stagingPath,
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8'
  );
  fs.renameSync(stagingPath, manifestPath);
  return manifestPath;
}

function createDevRuntimeError(
  stage: DevRuntimeStage,
  error: unknown,
  specifier?: string
): DevRuntimeError {
  const normalized = error instanceof Error ? error : new Error(String(error));
  return {
    stage,
    ...(specifier ? { specifier } : {}),
    name: normalized.name,
    message: normalized.message
  };
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
  manifest: DevGenerationDependencyManifest,
  afterReady?: () => void
) {
  if (receipt.state !== 'Building') transitionRuntime(receipt, 'Building');
  let stage: DevRuntimeStage = 'entry_transform';
  try {
    await Promise.all([
      server.transformRequest('/src/bootstrap.ts'),
      server.transformRequest('/src/main.tsx'),
      server.transformRequest('/src/app/App.tsx'),
      server.transformRequest('/src/app/ApplicationBootBoundary.tsx'),
      server.transformRequest('/src/app/ApplicationRuntimeBootstrap.tsx')
    ]);
    transitionRuntime(receipt, 'Validating');
    stage = 'optimizer_contract';
    await waitForCriticalInteropCache(server.config.cacheDir);
    stage = 'manifest_publish';
    persistDevGenerationDependencyManifest(server.config.cacheDir, manifest);
    receipt.error = null;
    receipt.readyAt = new Date().toISOString();
    transitionRuntime(receipt, 'Ready');
    afterReady?.();
  } catch (error: unknown) {
    receipt.error = createDevRuntimeError(stage, error);
    transitionRuntime(receipt, 'Degraded');
    server.config.logger.error(
      `[1flowbase-dev-runtime] ${stage} failed: ${receipt.error.message}`
    );
  }
}

function attachRecoveryProbeMiddleware(
  server: ViteDevServer,
  receipt: DevRuntimeReceipt,
  manifest: DevGenerationDependencyManifest,
  afterReady?: () => void
) {
  server.middlewares.use(RECOVERY_PROBE_PATH, (_request, response) => {
    receipt.error = createDevRuntimeError(
      'recovery',
      new Error('controlled recovery probe')
    );
    transitionRuntime(receipt, 'Degraded');
    response.statusCode = 202;
    response.setHeader('content-type', 'application/json; charset=utf-8');
    response.setHeader('cache-control', 'no-store');
    response.end(JSON.stringify({ state: receipt.state }));
    setImmediate(() => void warmRuntime(server, receipt, manifest, afterReady));
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
  const manifest = createDevGenerationDependencyManifest(root, mode);
  const generation = sha256(JSON.stringify(manifest));
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
        state: 'Building' as DevRuntimeState,
        cacheIdentity: generation,
        generation,
        startedAt: new Date().toISOString(),
        readyAt: null,
        error: null,
        transitions: [
          { state: 'Building' as DevRuntimeState, at: new Date().toISOString() }
        ]
      };
      attachReadinessMiddleware(server, receipt);
      if (hmrProbeFile) attachHmrProbeMiddleware(server, hmrProbeFile);
      attachRecoveryProbeMiddleware(
        server,
        receipt,
        manifest,
        retireStaleGenerations
      );
      attachPreReadyTrafficGate(server, receipt);
      server.httpServer?.once(
        'listening',
        () =>
          void warmRuntime(server, receipt, manifest, retireStaleGenerations)
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
  DEV_GENERATION_MANIFEST_NAME,
  DEV_GENERATION_META_NAME,
  HMR_PROBE_ID,
  HMR_PROBE_PATH,
  RECOVERY_PROBE_PATH,
  READY_PATH,
  createDevGenerationDependencyManifest,
  createDevRuntimeError,
  devCacheIdentity,
  devGenerationCacheDirectory,
  hmrProbeSource,
  oneFlowbaseDevRuntimePlugin,
  persistDevGenerationDependencyManifest,
  pruneDevGenerationCaches,
  verifyCriticalInteropCache,
  waitForCriticalInteropCache
};
