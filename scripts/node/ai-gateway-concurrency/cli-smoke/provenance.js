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
  codex: ['codex-rs/rust-toolchain.toml', 'codex-rs/rust-toolchain', 'codex-rs/Cargo.lock'],
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
    return [{
      name,
      kind: /lock/iu.test(name) ? 'lockfile' : 'toolchain-or-package-manifest',
      sha256: sha256File(filePath),
    }];
  });
}

function inspectSourceProvenance(client, input, dependencies = {}) {
  const runGit = dependencies.git || git;
  const expectedSha = FIXED_SOURCE_SHA[client];
  const actualSha = runGit(input.sourceRoot, ['rev-parse', 'HEAD']);
  const dirty = runGit(input.sourceRoot, ['status', '--porcelain', '--untracked-files=all']) !== '';
  const remoteUrl = runGit(input.sourceRoot, ['remote', 'get-url', 'origin']);
  const canonicalIdentity = (value) => value
    .replace(/^github:/u, '')
    .replace(/^https:\/\/github\.com\//u, '')
    .replace(/^git@github\.com:/u, '')
    .replace(/\.git$/u, '');
  let branchRef = null;
  try {
    branchRef = runGit(input.sourceRoot, ['symbolic-ref', '--quiet', 'HEAD']);
  } catch {
    // A detached HEAD is the required reproducible source state.
  }
  if (actualSha !== expectedSha) {
    throw new Error(`${client} source SHA ${actualSha} does not match fixed SHA ${expectedSha}`);
  }
  if (dirty) throw new Error(`${client} source worktree must be clean for provenance`);
  if (branchRef) throw new Error(`${client} source worktree must use a detached HEAD for provenance`);
  if (canonicalIdentity(remoteUrl) !== canonicalIdentity(input.sourceIdentity)) {
    throw new Error(`${client} source identity does not match origin remote`);
  }
  const identified = identifiedFiles(input.sourceRoot, SOURCE_FILES[client]);
  if (!identified.some((entry) => entry.kind === 'lockfile')
    || !identified.some((entry) => entry.kind === 'toolchain-or-package-manifest')) {
    throw new Error(`${client} source omitted required toolchain/package and lockfile provenance`);
  }
  return {
    schema_version: PROVENANCE_SCHEMA,
    client_kind: client,
    source: {
      kind: 'git-worktree',
      path: input.sourceRoot,
      identity: input.sourceIdentity,
      observed_remote: remoteUrl,
      fixed_revision: expectedSha,
      observed_revision: actualSha,
      dirty: false,
      detached: true,
    },
    toolchain_and_lockfiles: identified,
  };
}

function sourceBuiltProvenance(client, executable, input, dependencies = {}) {
  return {
    ...inspectSourceProvenance(client, input, dependencies),
    provenance_claim: 'source-built-from-fixed-git-commit',
    build_command: commandIdentity(input.buildCommand),
    executable: { path: executable, sha256: sha256File(executable) },
  };
}

function pinnedClaudeProvenance(executable, input) {
  const manifest = JSON.parse(fs.readFileSync(input.packageManifest, 'utf8'));
  if (manifest.name !== input.packageName || manifest.version !== input.packageVersion) {
    throw new Error('Claude package manifest identity does not match configured package identity');
  }
  return {
    schema_version: PROVENANCE_SCHEMA,
    client_kind: 'claude',
    provenance_claim: 'pinned-package-binary',
    source: { kind: 'package', dirty: null },
    package: {
      name: input.packageName,
      version: input.packageVersion,
      integrity: input.packageIntegrity,
      manifest: { path: input.packageManifest, sha256: sha256File(input.packageManifest) },
    },
    toolchain_and_lockfiles: [],
    installation_command: commandIdentity(input.installCommand),
    executable: { path: executable, sha256: sha256File(executable) },
  };
}

function collectClientProvenance(inputs, plans) {
  const invocationIdentities = (value) => Object.values(value.executable ? { default: value } : value)
    .map((plan) => commandIdentity([plan.executable, ...plan.args].join('\u0000')));
  return {
    codex: {
      ...sourceBuiltProvenance('codex', inputs.codexExecutable, inputs.provenance.codex),
      invocations: invocationIdentities(plans.codex),
    },
    claude: {
      ...pinnedClaudeProvenance(inputs.claudeExecutable, inputs.provenance.claude),
      invocations: invocationIdentities(plans.claude),
    },
    ...(plans.opencode ? {
      opencode: {
        ...sourceBuiltProvenance('opencode', inputs.opencodeExecutable, inputs.provenance.opencode),
        invocations: invocationIdentities(plans.opencode),
      },
    } : {}),
  };
}

module.exports = {
  FIXED_SOURCE_SHA,
  PROVENANCE_SCHEMA,
  collectClientProvenance,
  commandIdentity,
  inspectSourceProvenance,
  pinnedClaudeProvenance,
  sha256File,
  sourceBuiltProvenance,
};
