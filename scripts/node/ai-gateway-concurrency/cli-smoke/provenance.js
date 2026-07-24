'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const FIXED_SOURCE_SHA = Object.freeze({
  codex: '56395bddaf26eb2829387ca6a417bf9128e5b239',
  opencode: '411eff73f026d4950c07947c4d983788cb615baa',
});
const PROVENANCE_SCHEMA = '1flowbase.ai-gateway-client-provenance/v1';

const SOURCE_FILES = Object.freeze({
  codex: ['rust-toolchain.toml', 'rust-toolchain', 'Cargo.lock'],
  opencode: ['package.json', 'bun.lock', 'bun.lockb', 'pnpm-lock.yaml'],
});

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function commandIdentity(command) {
  return {
    schema_version: PROVENANCE_SCHEMA,
    command,
    sha256: crypto.createHash('sha256').update(command).digest('hex'),
  };
}

function git(root, args) {
  return execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' }).trim();
}

function identifiedFiles(root, candidates) {
  return candidates.flatMap((name) => {
    const filePath = path.join(root, name);
    if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) return [];
    return [{ name, sha256: sha256File(filePath) }];
  });
}

function sourceBuiltProvenance(client, executable, input, dependencies = {}) {
  const runGit = dependencies.git || git;
  const expectedSha = FIXED_SOURCE_SHA[client];
  const actualSha = runGit(input.sourceRoot, ['rev-parse', 'HEAD']);
  const dirty = runGit(input.sourceRoot, ['status', '--porcelain', '--untracked-files=all']) !== '';
  if (actualSha !== expectedSha) {
    throw new Error(`${client} source SHA ${actualSha} does not match fixed SHA ${expectedSha}`);
  }
  if (dirty) throw new Error(`${client} source worktree must be clean for provenance`);
  return {
    schema_version: PROVENANCE_SCHEMA,
    client_kind: client,
    provenance_claim: 'source-built-from-fixed-git-commit',
    source: {
      kind: 'git-worktree',
      identity: input.sourceIdentity,
      fixed_revision: expectedSha,
      observed_revision: actualSha,
      dirty: false,
    },
    toolchain_and_lockfiles: identifiedFiles(input.sourceRoot, SOURCE_FILES[client]),
    build_command: commandIdentity(input.buildCommand),
    executable: { sha256: sha256File(executable) },
  };
}

function pinnedClaudeProvenance(executable, input) {
  return {
    client_kind: 'claude',
    provenance_claim: 'pinned-package-binary',
    source: { kind: 'package', dirty: null },
    package: {
      name: input.packageName,
      version: input.packageVersion,
      integrity: input.packageIntegrity,
    },
    toolchain_and_lockfiles: [],
    build_command: commandIdentity(input.installCommand),
    executable: { sha256: sha256File(executable) },
  };
}

function collectClientProvenance(inputs, plans) {
  return {
    codex: {
      ...sourceBuiltProvenance('codex', inputs.codexExecutable, inputs.provenance.codex),
      invocation: commandIdentity([plans.codex.executable, ...plans.codex.args].join('\u0000')),
    },
    claude: {
      ...pinnedClaudeProvenance(inputs.claudeExecutable, inputs.provenance.claude),
      invocation: commandIdentity([plans.claude.executable, ...plans.claude.args].join('\u0000')),
    },
    ...(plans.opencode ? {
      opencode: {
        ...sourceBuiltProvenance('opencode', inputs.opencodeExecutable, inputs.provenance.opencode),
        invocation: commandIdentity([plans.opencode.executable, ...plans.opencode.args].join('\u0000')),
      },
    } : {}),
  };
}

module.exports = {
  FIXED_SOURCE_SHA,
  PROVENANCE_SCHEMA,
  collectClientProvenance,
  commandIdentity,
  pinnedClaudeProvenance,
  sha256File,
  sourceBuiltProvenance,
};
