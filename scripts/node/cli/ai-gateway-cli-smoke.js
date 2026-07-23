#!/usr/bin/env node
'use strict';

const { runCliSmoke } = require('../ai-gateway-concurrency/cli-smoke');

const FIELDS = new Map([
  ['--ready-manifest', 'readyManifest'],
  ['--codex-executable', 'codexExecutable'],
  ['--claude-executable', 'claudeExecutable'],
  ['--opencode-executable', 'opencodeExecutable'],
  ['--first-marker', 'firstMarker'],
  ['--second-marker', 'secondMarker'],
  ['--barrier-release-url', 'barrierReleaseUrl'],
]);

function usage() {
  return `Usage: node scripts/node/cli/ai-gateway-cli-smoke.js \\
  --ready-manifest <WP3-ready.json> \\
  --codex-executable <path> --claude-executable <path> [--opencode-executable <path>] \\
  [--tmux-timing --first-marker <delta-1> --second-marker <delta-2> \\
   --barrier-release-url <loopback-url>]

The values may instead be supplied as AI_GATEWAY_FIXTURE_READY_FILE,
AI_GATEWAY_CODEX_EXECUTABLE, AI_GATEWAY_CLAUDE_EXECUTABLE, and
AI_GATEWAY_OPENCODE_EXECUTABLE. With --tmux-timing, each client runs inside an isolated
tmux + util-linux script PTY and evidence is written below
tmp/test-governance/compatible-stream-e2e/<run-id>/.`;
}

function parseArgs(argv, env = process.env) {
  const options = {
    readyManifest: env.AI_GATEWAY_FIXTURE_READY_FILE,
    codexExecutable: env.AI_GATEWAY_CODEX_EXECUTABLE,
    claudeExecutable: env.AI_GATEWAY_CLAUDE_EXECUTABLE,
    opencodeExecutable: env.AI_GATEWAY_OPENCODE_EXECUTABLE,
    tmuxTiming: env.AI_GATEWAY_TMUX_TIMING === '1',
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--help' || flag === '-h') return { help: true };
    if (flag === '--tmux-timing') {
      options.tmuxTiming = true;
      continue;
    }
    const field = FIELDS.get(flag);
    if (!field) throw new Error(`unknown option ${flag}`);
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${flag} requires a value`);
    options[field] = value;
    index += 1;
  }
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(`${usage()}\n`);
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
