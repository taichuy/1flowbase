#!/usr/bin/env node
'use strict';

const { runCliSmoke } = require('../ai-gateway-concurrency/cli-smoke');
const {
  runClaudeRepositoryHistorySmoke,
} = require('../ai-gateway-concurrency/cli-smoke/repository-history');

const FIELDS = new Map([
  ['--ready-manifest', 'readyManifest'],
  ['--codex-executable', 'codexExecutable'],
  ['--claude-executable', 'claudeExecutable'],
  ['--opencode-executable', 'opencodeExecutable'],
  ['--secret-canary', 'secretCanary'],
  ['--codex-source-root', 'codexSourceRoot'],
  ['--codex-source-identity', 'codexSourceIdentity'],
  ['--codex-build-command', 'codexBuildCommand'],
  ['--claude-package-name', 'claudePackageName'],
  ['--claude-package-manifest', 'claudePackageManifest'],
  ['--claude-package-version', 'claudePackageVersion'],
  ['--claude-package-integrity', 'claudePackageIntegrity'],
  ['--claude-install-command', 'claudeInstallCommand'],
  ['--opencode-source-root', 'opencodeSourceRoot'],
  ['--opencode-source-identity', 'opencodeSourceIdentity'],
  ['--opencode-build-command', 'opencodeBuildCommand'],
  ['--repository', 'repository'],
  ['--model', 'model'],
  ['--base-url', 'baseUrl'],
  ['--proxy-url', 'proxyUrl'],
  ['--evidence-root', 'evidenceRoot'],
  ['--timeout-ms', 'timeoutMs'],
]);

function usage() {
  const provenance = [
    'Required provenance: --codex-source-root, --codex-source-identity, --codex-build-command,',
    '--claude-package-manifest, --claude-package-name, --claude-package-version, --claude-package-integrity,',
    '--claude-install-command, and (with OpenCode) --opencode-source-root plus',
    '--opencode-source-identity and --opencode-build-command. The ready manifest supplies',
    'the controlled upstream timeline, barrier, network, and executor observers.',
  ].join(' ');
  return `Usage: node scripts/node/cli/ai-gateway-cli-smoke.js \\
  --ready-manifest <WP3-ready.json> \\
  --codex-executable <path> --claude-executable <path> [--opencode-executable <path>] \\
  [--tmux-timing] [--secret-canary <canary>]

The values may instead be supplied as AI_GATEWAY_FIXTURE_READY_FILE,
AI_GATEWAY_CODEX_EXECUTABLE, AI_GATEWAY_CLAUDE_EXECUTABLE, and
AI_GATEWAY_OPENCODE_EXECUTABLE. With --tmux-timing, each client runs inside an isolated
tmux pipe-pane PTY stream (with supplementary capture-pane and util-linux timing) writes evidence below
tmp/test-governance/compatible-stream-e2e/<run-id>/.

Native Claude repository history mode:
  --claude-repository-history --repository <git-repo> --model <claude-model> \\
  [--claude-executable <path>] [--base-url <url>] [--proxy-url <url>] \\
  [--evidence-root <dir>]

This mode reads ANTHROPIC_BASE_URL plus ANTHROPIC_AUTH_TOKEN or ANTHROPIC_API_KEY
from the environment, runs only local Claude Code inside tmux, requires a completed
read-only Bash git-log call, and verifies the exact latest three commits at runtime.

${provenance}`;
}

function parseArgs(argv, env = process.env) {
  const options = {
    readyManifest: env.AI_GATEWAY_FIXTURE_READY_FILE,
    codexExecutable: env.AI_GATEWAY_CODEX_EXECUTABLE,
    claudeExecutable: env.AI_GATEWAY_CLAUDE_EXECUTABLE,
    opencodeExecutable: env.AI_GATEWAY_OPENCODE_EXECUTABLE,
    tmuxTiming: env.AI_GATEWAY_TMUX_TIMING === '1',
    codexSourceRoot: env.AI_GATEWAY_CODEX_SOURCE_ROOT,
    codexSourceIdentity: env.AI_GATEWAY_CODEX_SOURCE_IDENTITY,
    codexBuildCommand: env.AI_GATEWAY_CODEX_BUILD_COMMAND,
    claudePackageName: env.AI_GATEWAY_CLAUDE_PACKAGE_NAME,
    claudePackageManifest: env.AI_GATEWAY_CLAUDE_PACKAGE_MANIFEST,
    claudePackageVersion: env.AI_GATEWAY_CLAUDE_PACKAGE_VERSION,
    claudePackageIntegrity: env.AI_GATEWAY_CLAUDE_PACKAGE_INTEGRITY,
    claudeInstallCommand: env.AI_GATEWAY_CLAUDE_INSTALL_COMMAND,
    opencodeSourceRoot: env.AI_GATEWAY_OPENCODE_SOURCE_ROOT,
    opencodeSourceIdentity: env.AI_GATEWAY_OPENCODE_SOURCE_IDENTITY,
    opencodeBuildCommand: env.AI_GATEWAY_OPENCODE_BUILD_COMMAND,
    secretCanary: env.AI_GATEWAY_SECRET_CANARY,
    claudeRepositoryHistory: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--help' || flag === '-h') return { help: true };
    if (flag === '--tmux-timing') {
      options.tmuxTiming = true;
      continue;
    }
    if (flag === '--claude-repository-history') {
      options.claudeRepositoryHistory = true;
      continue;
    }
    const field = FIELDS.get(flag);
    if (!field) throw new Error(`unknown option ${flag}`);
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${flag} requires a value`);
    options[field] = value;
    index += 1;
  }
  if (options.claudeRepositoryHistory) {
    const repositoryOptions = {
      claudeRepositoryHistory: true,
      repository: options.repository,
      model: options.model,
      evidenceRoot: options.evidenceRoot,
      timeoutMs: options.timeoutMs === undefined ? undefined : Number(options.timeoutMs),
      claudeExecutable: options.claudeExecutable,
      baseUrl: options.baseUrl,
      proxyUrl: options.proxyUrl,
    };
    return Object.fromEntries(Object.entries(repositoryOptions).filter(([, value]) => value !== undefined));
  }
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(`${usage()}\n`);
    return;
  }
  if (options.claudeRepositoryHistory) {
    const result = await runClaudeRepositoryHistorySmoke(options);
    process.stdout.write(`${JSON.stringify(result)}\n`);
    if (!result.ok) process.exitCode = 2;
    return;
  }
  const result = await runCliSmoke(options);
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`[ai-gateway-cli-smoke] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { parseArgs, usage };
