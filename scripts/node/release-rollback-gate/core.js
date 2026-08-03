const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const REPORT_JSON_FILE = 'release-rollback-gate.json';
const REPORT_MARKDOWN_FILE = 'release-rollback-gate.md';
const SNAPSHOT_FILE = 'release-rollback-db.snapshot.dump';
const COMPOSE_FILE = 'release-rollback-compose.yml';
const COMPOSE_LOG_FILE = 'release-rollback-compose.log';
const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_WEB_PORT = 39_100;
const PROVIDER_SECRET_MASTER_KEY = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const ROOT_PASSWORD = 'rollback-gate-root-password';
const PASSWORD_LOCAL_AUTHENTICATOR_ID = '00000000-0000-0000-0000-000000000001';
const DATABASE_NAME = '1flowbase';
const DATABASE_USER = 'postgres';
const DATABASE_PASSWORD = '1flowbase';

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function sanitizeProjectName(value) {
  const normalized = String(value || '')
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/gu, '-')
    .replace(/^-+|-+$/gu, '')
    .slice(0, 58);

  return normalized || 'release-rollback';
}

function parsePositiveInt(value, label) {
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || String(parsed) !== String(value)) {
    throw new Error(`${label} must be a positive integer`);
  }
  return parsed;
}

function repositoryOwnerFromEnv(env = process.env) {
  if (env.GITHUB_REPOSITORY_OWNER) {
    return env.GITHUB_REPOSITORY_OWNER;
  }

  if (env.GITHUB_REPOSITORY && env.GITHUB_REPOSITORY.includes('/')) {
    return env.GITHUB_REPOSITORY.split('/')[0];
  }

  return 'taichuy';
}

function defaultProjectName(env = process.env) {
  return sanitizeProjectName(`release-rollback-${env.GITHUB_RUN_ID || 'local'}-${env.GITHUB_RUN_ATTEMPT || '1'}`);
}

function parseCliArgs(argv = [], env = process.env) {
  const options = {
    candidateImageTag: env.CANDIDATE_IMAGE_TAG || 'latest',
    help: false,
    outputRoot: env.OUTPUT_ROOT || OUTPUT_ROOT,
    previousImageTag: env.PREVIOUS_IMAGE_TAG || 'auto',
    projectName: env.COMPOSE_PROJECT_NAME || defaultProjectName(env),
    repositoryOwner: env.GITHUB_REPOSITORY_OWNER || repositoryOwnerFromEnv(env),
    timeoutMs: DEFAULT_TIMEOUT_MS,
    webPort: DEFAULT_WEB_PORT,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    const readValue = (label) => {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error(`${label} requires a value`);
      }
      index += 1;
      return value;
    };

    if (arg === '--previous-image-tag') {
      options.previousImageTag = readValue(arg);
      continue;
    }

    if (arg === '--candidate-image-tag') {
      options.candidateImageTag = readValue(arg);
      continue;
    }

    if (arg === '--repository-owner') {
      options.repositoryOwner = readValue(arg);
      continue;
    }

    if (arg === '--project-name') {
      options.projectName = sanitizeProjectName(readValue(arg));
      continue;
    }

    if (arg === '--output-root') {
      options.outputRoot = readValue(arg);
      continue;
    }

    if (arg === '--web-port') {
      options.webPort = parsePositiveInt(readValue(arg), '--web-port');
      continue;
    }

    if (arg === '--timeout-ms') {
      options.timeoutMs = parsePositiveInt(readValue(arg), '--timeout-ms');
      continue;
    }

    throw new Error(`Unknown option: ${arg}`);
  }

  options.projectName = sanitizeProjectName(options.projectName);
  return options;
}

function runCommand(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd || process.cwd(),
    env: options.env || process.env,
    input: options.input,
    encoding: options.encoding || 'utf8',
    maxBuffer: options.maxBuffer || 128 * 1024 * 1024,
    stdio: options.stdio || ['pipe', 'pipe', 'pipe'],
  });
}

function ensureCommandSuccess(result, label) {
  if (result.error) {
    throw new Error(`${label} failed: ${result.error.message}`);
  }

  if (result.status !== 0) {
    const stderr = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString('utf8')
      : String(result.stderr || '');
    const stdout = Buffer.isBuffer(result.stdout)
      ? result.stdout.toString('utf8')
      : String(result.stdout || '');
    const detail = (stderr || stdout).trim();
    throw new Error(`${label} failed with exit ${result.status}${detail ? `: ${detail}` : ''}`);
  }

  return result;
}

function resolvePreviousImageTag({
  requestedTag,
  runCommandImpl = runCommand,
} = {}) {
  if (requestedTag && requestedTag !== 'auto') {
    return requestedTag;
  }

  const result = runCommandImpl('gh', ['release', 'list', '--limit', '2', '--json', 'tagName']);
  ensureCommandSuccess(result, 'resolve previous release tag');

  let releases;
  try {
    releases = JSON.parse(result.stdout || '[]');
  } catch (error) {
    throw new Error(`resolve previous release tag failed: invalid gh JSON (${error.message})`);
  }

  const tag = releases?.[1]?.tagName || releases?.[0]?.tagName;
  if (!tag) {
    throw new Error('resolve previous release tag failed: no GitHub releases found');
  }

  return tag;
}

function buildComposeContent({ repositoryOwner, webPort }) {
  return `name: 1flowbase-release-rollback

services:
  db:
    image: postgres:18-alpine
    shm_size: 512m
    command: ["postgres", "-c", "max_connections=100"]
    environment:
      POSTGRES_DB: ${DATABASE_NAME}
      POSTGRES_USER: ${DATABASE_USER}
      POSTGRES_PASSWORD: ${DATABASE_PASSWORD}
      POSTGRES_INITDB_ARGS: "--encoding=UTF8 --locale=C"
    volumes:
      - rollback-db-data:/var/lib/postgresql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${DATABASE_USER} -d ${DATABASE_NAME}"]
      interval: 5s
      timeout: 3s
      retries: 20

  plugin-runner:
    image: ghcr.io/${repositoryOwner}/1flowbase-plugin-runner:\${FLOWBASE_PLUGIN_RUNNER_VERSION}
    environment:
      RUST_LOG: info
      PLUGIN_RUNNER_ADDR: 0.0.0.0:7801
    expose:
      - "7801"

  api:
    image: ghcr.io/${repositoryOwner}/1flowbase-api-server:\${FLOWBASE_API_SERVER_VERSION}
    depends_on:
      db:
        condition: service_healthy
      plugin-runner:
        condition: service_started
    environment:
      RUST_LOG: info
      API_ENV: development
      API_DATABASE_URL: postgres://${DATABASE_USER}:${DATABASE_PASSWORD}@db:5432/${DATABASE_NAME}
      API_SERVER_ADDR: 0.0.0.0:7800
      API_PLUGIN_RUNNER_INTERNAL_BASE_URL: http://plugin-runner:7801
      API_ALLOWED_ORIGINS: http://localhost:${webPort},http://127.0.0.1:${webPort}
      API_COOKIE_NAME: flowbase_console_session
      API_SESSION_TTL_DAYS: "7"
      API_DATABASE_POOL_MAX_CONNECTIONS: "5"
      API_PROVIDER_INSTALL_ROOT: /app/api/plugins
      API_HOST_EXTENSION_DROPIN_ROOT: /app/api/plugins/host-extension/dropins
      API_PROVIDER_SECRET_MASTER_KEY: ${PROVIDER_SECRET_MASTER_KEY}
      API_OFFICIAL_PLUGIN_REPOSITORY: taichuy/1flowbase-official-plugins
      API_OFFICIAL_PLUGIN_REGISTRY_URL: https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/official-registry.json
      API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL: ""
      API_PLUGIN_SET: default
      API_SECRET_RESOLVER: env
      API_PLUGIN_ALLOW_UNVERIFIED_FILESYSTEM_DROPINS: "false"
      API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS: "false"
      BOOTSTRAP_WORKSPACE_NAME: 1flowbase
      BOOTSTRAP_ROOT_ACCOUNT: root
      BOOTSTRAP_ROOT_EMAIL: root@example.com
      BOOTSTRAP_ROOT_PASSWORD: ${ROOT_PASSWORD}
      BOOTSTRAP_ROOT_NAME: Root
      BOOTSTRAP_ROOT_NICKNAME: Root
    expose:
      - "7800"
    volumes:
      - rollback-api-storage:/app/api/storage
      - rollback-plugin-packages:/app/api/plugins/packages
      - rollback-plugin-installed:/app/api/plugins/installed
      - rollback-host-dropins:/app/api/plugins/host-extension/dropins

  web:
    image: ghcr.io/${repositoryOwner}/1flowbase-web:\${FLOWBASE_WEB_VERSION}
    depends_on:
      api:
        condition: service_started
    ports:
      - "${webPort}:80"

volumes:
  rollback-db-data:
  rollback-api-storage:
  rollback-plugin-packages:
  rollback-plugin-installed:
  rollback-host-dropins:
`;
}

function imageEnv(imageTag, baseEnv = process.env) {
  return {
    ...baseEnv,
    FLOWBASE_WEB_VERSION: imageTag,
    FLOWBASE_API_SERVER_VERSION: imageTag,
    FLOWBASE_PLUGIN_RUNNER_VERSION: imageTag,
  };
}

function composeArgs({ composeFile, projectName }, args) {
  return ['compose', '-p', projectName, '-f', composeFile, ...args];
}

function buildRollbackGatePlan({
  candidateImageTag,
  composeFile,
  previousImageTag,
  projectName,
  snapshotPath,
  webPort,
}) {
  const compose = { composeFile, projectName };
  const baseUrl = `http://127.0.0.1:${webPort}`;

  return [
    {
      id: 'pull-previous-images',
      label: 'Pull previous release images',
      kind: 'command',
      command: 'docker',
      args: composeArgs(compose, ['pull']),
      imageTag: previousImageTag,
    },
    {
      id: 'start-previous-baseline',
      label: 'Start previous release baseline',
      kind: 'command',
      command: 'docker',
      args: composeArgs(compose, ['up', '-d', '--wait']),
      imageTag: previousImageTag,
    },
    {
      id: 'smoke-previous-baseline',
      label: 'Smoke previous release baseline',
      kind: 'http-smoke',
      baseUrl,
      imageTag: previousImageTag,
    },
    {
      id: 'create-db-snapshot',
      label: 'Create DB snapshot',
      kind: 'snapshot',
      compose,
      snapshotPath,
      imageTag: previousImageTag,
    },
    {
      id: 'pull-candidate-images',
      label: 'Pull candidate images',
      kind: 'command',
      command: 'docker',
      args: composeArgs(compose, ['pull']),
      imageTag: candidateImageTag,
    },
    {
      id: 'start-candidate',
      label: 'Start candidate image',
      kind: 'command',
      command: 'docker',
      args: composeArgs(compose, ['up', '-d', '--wait']),
      imageTag: candidateImageTag,
    },
    {
      id: 'smoke-candidate',
      label: 'Smoke candidate image',
      kind: 'http-smoke',
      baseUrl,
      imageTag: candidateImageTag,
    },
    {
      id: 'restore-db-snapshot',
      label: 'Restore DB snapshot',
      kind: 'restore',
      compose,
      snapshotPath,
      imageTag: previousImageTag,
    },
    {
      id: 'restart-previous-after-restore',
      label: 'Restart previous release after restore',
      kind: 'command',
      command: 'docker',
      args: composeArgs(compose, ['up', '-d', '--wait']),
      imageTag: previousImageTag,
    },
    {
      id: 'smoke-previous-after-restore',
      label: 'Smoke previous release after restore',
      kind: 'http-smoke',
      baseUrl,
      imageTag: previousImageTag,
    },
  ];
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      accept: 'application/json',
      ...(options.body ? { 'content-type': 'application/json' } : {}),
      ...(options.headers || {}),
    },
    signal: AbortSignal.timeout(options.timeoutMs || 10_000),
  });

  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch (_error) {
    json = null;
  }

  return { response, json, text };
}

async function waitForHealth({
  baseUrl,
  timeoutMs,
  fetchJsonImpl = fetchJson,
  sleepImpl = sleep,
}) {
  const startedAt = Date.now();
  let lastError = '';

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const { response, json, text } = await fetchJsonImpl(`${baseUrl}/health`, { timeoutMs: 5_000 });
      if (response.ok && json?.status === 'ok') {
        return json;
      }
      lastError = `health returned ${response.status}: ${text.slice(0, 160)}`;
    } catch (error) {
      lastError = error.message;
    }

    await sleepImpl(2_000);
  }

  throw new Error(`health did not become ready: ${lastError}`);
}

async function runHttpSmoke({
  baseUrl,
  timeoutMs,
  fetchJsonImpl = fetchJson,
  sleepImpl = sleep,
}) {
  const health = await waitForHealth({
    baseUrl,
    timeoutMs,
    fetchJsonImpl,
    sleepImpl,
  });
  const providers = await fetchJsonImpl(`${baseUrl}/api/public/auth/providers`, { timeoutMs: 10_000 });
  if (!providers.response.ok) {
    throw new Error(`provider list failed with ${providers.response.status}: ${providers.text.slice(0, 160)}`);
  }

  const login = await fetchJsonImpl(`${baseUrl}/api/public/auth/sign-in`, {
    method: 'POST',
    body: JSON.stringify({
      authenticator_id: PASSWORD_LOCAL_AUTHENTICATOR_ID,
      identifier: 'root',
      password: ROOT_PASSWORD,
    }),
    timeoutMs: 10_000,
  });

  if (!login.response.ok || !login.json?.data?.current_workspace_id) {
    throw new Error(`root login failed with ${login.response.status}: ${login.text.slice(0, 160)}`);
  }

  return {
    health,
    currentWorkspaceId: login.json.data.current_workspace_id,
  };
}

function runComposeCommand({ compose, args, imageTag, repoRoot, runCommandImpl }) {
  return ensureCommandSuccess(
    runCommandImpl('docker', composeArgs(compose, args), {
      cwd: repoRoot,
      env: imageEnv(imageTag),
    }),
    `docker compose ${args.join(' ')}`
  );
}

function createSnapshot({ compose, imageTag, repoRoot, runCommandImpl, snapshotPath }) {
  const result = ensureCommandSuccess(
    runCommandImpl('docker', composeArgs(compose, [
      'exec',
      '-T',
      'db',
      'pg_dump',
      '-U',
      DATABASE_USER,
      '-d',
      DATABASE_NAME,
      '-Fc',
    ]), {
      cwd: repoRoot,
      env: imageEnv(imageTag),
      encoding: 'buffer',
      maxBuffer: 128 * 1024 * 1024,
    }),
    'create DB snapshot'
  );

  fs.writeFileSync(snapshotPath, result.stdout);
}

function quoteSqlIdentifier(value) {
  return `"${String(value).replace(/"/gu, '""')}"`;
}

function restoreSnapshot({ compose, imageTag, repoRoot, runCommandImpl, snapshotPath }) {
  const snapshot = fs.readFileSync(snapshotPath);

  runComposeCommand({
    compose,
    args: ['stop', 'web', 'api', 'plugin-runner'],
    imageTag,
    repoRoot,
    runCommandImpl,
  });

  ensureCommandSuccess(
    runCommandImpl('docker', composeArgs(compose, [
      'exec',
      '-T',
      'db',
      'psql',
      '-U',
      DATABASE_USER,
      '-d',
      'postgres',
      '-v',
      'ON_ERROR_STOP=1',
      '-c',
      `SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '${DATABASE_NAME}' AND pid <> pg_backend_pid();`,
      '-c',
      `DROP DATABASE IF EXISTS ${quoteSqlIdentifier(DATABASE_NAME)};`,
      '-c',
      `CREATE DATABASE ${quoteSqlIdentifier(DATABASE_NAME)};`,
    ]), {
      cwd: repoRoot,
      env: imageEnv(imageTag),
    }),
    'recreate DB before restore'
  );

  ensureCommandSuccess(
    runCommandImpl('docker', composeArgs(compose, [
      'exec',
      '-T',
      'db',
      'pg_restore',
      '-U',
      DATABASE_USER,
      '-d',
      DATABASE_NAME,
      '--no-owner',
    ]), {
      cwd: repoRoot,
      env: imageEnv(imageTag),
      input: snapshot,
      maxBuffer: 128 * 1024 * 1024,
    }),
    'restore DB snapshot'
  );
}

function stepResult({ step, status, startedAt, error, evidence }) {
  return {
    id: step.id,
    label: step.label,
    status,
    imageTag: step.imageTag || '',
    durationMs: Date.now() - startedAt,
    error: error ? error.message : '',
    evidence: evidence || null,
  };
}

function formatMarkdownReport(report) {
  const rows = report.steps.map((step) => (
    `| \`${step.id}\` | ${step.status} | \`${step.imageTag || 'n/a'}\` | ${(step.durationMs / 1000).toFixed(2)}s | ${step.error || ''} |`
  ));

  return [
    '# Release Rollback Gate',
    '',
    '## Summary',
    '',
    `- Status: ${report.status}`,
    `- Exit code: ${report.exitCode}`,
    `- Previous image tag: ${report.previousImageTag}`,
    `- Candidate image tag: ${report.candidateImageTag}`,
    `- Web base URL: ${report.webBaseUrl}`,
    `- Snapshot: ${report.snapshotPath}`,
    `- Compose log: ${report.composeLogPath}`,
    '',
    '## Steps',
    '',
    '| Step | Status | Image tag | Duration | Error |',
    '| --- | --- | --- | ---: | --- |',
    ...rows,
    '',
  ].join('\n');
}

function writeReport({ outputDir, report }) {
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(path.join(outputDir, REPORT_JSON_FILE), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  fs.writeFileSync(path.join(outputDir, REPORT_MARKDOWN_FILE), formatMarkdownReport(report), 'utf8');
}

function collectComposeLogs({ compose, imageTag, repoRoot, runCommandImpl, outputDir }) {
  const result = runCommandImpl('docker', composeArgs(compose, ['logs', '--no-color']), {
    cwd: repoRoot,
    env: imageEnv(imageTag),
    maxBuffer: 32 * 1024 * 1024,
  });
  const stdout = Buffer.isBuffer(result.stdout) ? result.stdout.toString('utf8') : String(result.stdout || '');
  const stderr = Buffer.isBuffer(result.stderr) ? result.stderr.toString('utf8') : String(result.stderr || '');
  fs.writeFileSync(path.join(outputDir, COMPOSE_LOG_FILE), `${stdout}${stderr}`, 'utf8');
}

async function runRollbackGate({
  candidateImageTag,
  outputRoot,
  previousImageTag,
  projectName,
  repositoryOwner,
  timeoutMs,
  webPort,
}, deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const runCommandImpl = deps.runCommandImpl || runCommand;
  const outputDir = path.isAbsolute(outputRoot) ? outputRoot : path.join(repoRoot, outputRoot);
  const resolvedPreviousImageTag = resolvePreviousImageTag({
    requestedTag: previousImageTag,
    runCommandImpl,
  });
  const composeFile = path.join(outputDir, COMPOSE_FILE);
  const snapshotPath = path.join(outputDir, SNAPSHOT_FILE);
  const compose = { composeFile, projectName };
  const steps = [];
  const reportBase = {
    candidateImageTag,
    composeLogPath: path.join(outputRoot, COMPOSE_LOG_FILE).replace(/\\/gu, '/'),
    generatedAt: new Date().toISOString(),
    previousImageTag: resolvedPreviousImageTag,
    snapshotPath: path.join(outputRoot, SNAPSHOT_FILE).replace(/\\/gu, '/'),
    webBaseUrl: `http://127.0.0.1:${webPort}`,
  };

  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(composeFile, buildComposeContent({ repositoryOwner, webPort }), 'utf8');

  const plan = buildRollbackGatePlan({
    candidateImageTag,
    composeFile,
    previousImageTag: resolvedPreviousImageTag,
    projectName,
    repositoryOwner,
    snapshotPath,
    webPort,
  });

  let exitCode = 0;
  try {
    for (const step of plan) {
      const startedAt = Date.now();
      try {
        let evidence = null;
        if (step.kind === 'command') {
          ensureCommandSuccess(
            runCommandImpl(step.command, step.args, {
              cwd: repoRoot,
              env: imageEnv(step.imageTag),
            }),
            step.label
          );
        } else if (step.kind === 'http-smoke') {
          evidence = await runHttpSmoke({
            baseUrl: step.baseUrl,
            timeoutMs,
          });
        } else if (step.kind === 'snapshot') {
          createSnapshot({
            compose: step.compose,
            imageTag: step.imageTag,
            repoRoot,
            runCommandImpl,
            snapshotPath: step.snapshotPath,
          });
        } else if (step.kind === 'restore') {
          restoreSnapshot({
            compose: step.compose,
            imageTag: step.imageTag,
            repoRoot,
            runCommandImpl,
            snapshotPath: step.snapshotPath,
          });
        } else {
          throw new Error(`unknown rollback step kind: ${step.kind}`);
        }
        steps.push(stepResult({ step, status: 'passed', startedAt, evidence }));
      } catch (error) {
        steps.push(stepResult({ step, status: 'failed', startedAt, error }));
        throw error;
      }
    }
  } catch (error) {
    exitCode = 1;
  } finally {
    try {
      collectComposeLogs({
        compose,
        imageTag: resolvedPreviousImageTag,
        repoRoot,
        runCommandImpl,
        outputDir,
      });
    } catch (_error) {
      // Logs are evidence only; the failing step already carries the blocker.
    }

    try {
      runCommandImpl('docker', composeArgs(compose, ['down', '--remove-orphans', '--volumes']), {
        cwd: repoRoot,
        env: imageEnv(resolvedPreviousImageTag),
      });
    } catch (_error) {
      // Cleanup is best-effort after report evidence has been captured.
    }

    writeReport({
      outputDir,
      report: {
        ...reportBase,
        exitCode,
        status: exitCode === 0 ? 'passed' : 'failed',
        steps,
      },
    });
  }

  return exitCode;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`Usage: node scripts/node/release-rollback-gate.js [options]

Options:
  --previous-image-tag <tag>   Previous release image tag, or auto. Default: auto
  --candidate-image-tag <tag>  Candidate image tag. Default: latest
  --repository-owner <owner>   GHCR owner. Default: GITHUB_REPOSITORY owner or taichuy
  --project-name <name>        Docker compose project name.
  --web-port <port>            Host port for web smoke. Default: ${DEFAULT_WEB_PORT}
  --timeout-ms <ms>            Per-smoke timeout. Default: ${DEFAULT_TIMEOUT_MS}
  --output-root <path>         Evidence directory. Default: ${OUTPUT_ROOT}
  -h, --help                   Show this help.
`);
}

module.exports = {
  buildComposeContent,
  buildRollbackGatePlan,
  parseCliArgs,
  resolvePreviousImageTag,
  runHttpSmoke,
  runRollbackGate,
  usage,
};
