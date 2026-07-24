'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  FIXED_SOURCE_SHA,
  pinnedClaudeProvenance,
  sourceBuiltProvenance,
} = require('../provenance');

// D3-AC-007: the fixed source identity and dirty=false gate are evidence, not labels.
test('source-built provenance fixes source SHA, lock/toolchain digests, command, and executable', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'd3-provenance-'));
  try {
    const executable = path.join(root, 'codex');
    fs.writeFileSync(executable, 'fixed binary fixture');
    fs.mkdirSync(path.join(root, 'codex-rs'));
    fs.writeFileSync(path.join(root, 'codex-rs', 'Cargo.lock'), 'fixed lock fixture');
    fs.writeFileSync(path.join(root, 'codex-rs', 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.88"\n');
    const cleanGit = (_cwd, args) => args[0] === 'rev-parse'
      ? FIXED_SOURCE_SHA.codex
      : args.length === 1 && args[0] === 'remote'
        ? 'origin'
        : args[0] === 'remote' ? 'https://github.com/openai/codex.git' : '';
    const value = sourceBuiltProvenance('codex', executable, {
      sourceRoot: root,
      sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: cleanGit });
    assert.equal(value.source.observed_revision, FIXED_SOURCE_SHA.codex);
    assert.equal(value.source.dirty, false);
    assert.equal(value.provenance_claim, 'source-built-from-fixed-git-commit');
    assert.deepEqual(value.toolchain_and_lockfiles.map((entry) => entry.name), [
      'codex-rs/rust-toolchain.toml', 'codex-rs/Cargo.lock',
    ]);
    assert.match(value.executable.sha256, /^[a-f0-9]{64}$/u);
    const upstreamOnly = sourceBuiltProvenance('codex', executable, {
      sourceRoot: root,
      sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: (_cwd, args) => {
      if (args[0] === 'rev-parse') return FIXED_SOURCE_SHA.codex;
      if (args[0] === 'status' || args[0] === 'symbolic-ref') return '';
      if (args.length === 1 && args[0] === 'remote') return 'upstream';
      if (args.join(' ') === 'remote get-url upstream') return 'https://github.com/openai/codex.git';
      throw new Error(`unexpected git args: ${args.join(' ')}`);
    } });
    assert.equal(upstreamOnly.source.observed_remote_name, 'upstream');
    assert.equal(upstreamOnly.source.observed_remote, 'https://github.com/openai/codex.git');
    assert.throws(() => sourceBuiltProvenance('codex', executable, {
      sourceRoot: root,
      sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: (_cwd, args) => {
      if (args[0] === 'rev-parse') return FIXED_SOURCE_SHA.codex;
      if (args[0] === 'status' || args[0] === 'symbolic-ref') return '';
      if (args.length === 1 && args[0] === 'remote') return 'upstream';
      if (args.join(' ') === 'remote get-url upstream') return 'https://github.com/example/fork.git';
      throw new Error(`unexpected git args: ${args.join(' ')}`);
    } }), /source identity does not match any configured remote/u);
    assert.throws(() => sourceBuiltProvenance('codex', executable, {
      sourceRoot: root, sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: (_cwd, args) => args[0] === 'rev-parse'
      ? FIXED_SOURCE_SHA.codex
      : args[0] === 'remote' ? 'https://github.com/openai/codex.git'
        : args[0] === 'symbolic-ref' ? 'refs/heads/main' : '' }), /detached HEAD/u);
    assert.throws(() => sourceBuiltProvenance('codex', executable, {
      sourceRoot: root, sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: (_cwd, args) => args[0] === 'rev-parse'
      ? FIXED_SOURCE_SHA.codex
      : args[0] === 'remote' ? 'https://github.com/openai/codex.git' : ' M Cargo.lock' }), /clean/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('Claude provenance is only a configurable pinned-package binary claim', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'd3-claude-provenance-'));
  try {
    const executable = path.join(root, 'claude');
    const packageManifest = path.join(root, 'package.json');
    fs.writeFileSync(executable, 'fixed package binary fixture');
    fs.writeFileSync(packageManifest, JSON.stringify({
      name: '@anthropic-ai/claude-code', version: 'configured-version',
    }));
    const value = pinnedClaudeProvenance(executable, {
      packageManifest,
      packageName: '@anthropic-ai/claude-code',
      packageVersion: 'configured-version',
      packageIntegrity: 'sha512-configured-integrity',
      installCommand: 'npm install --global @anthropic-ai/claude-code@configured-version',
    });
    assert.equal(value.provenance_claim, 'pinned-package-binary');
    assert.equal(value.source.kind, 'package');
    assert.equal(value.source.dirty, null);
    assert.equal(value.package.version, 'configured-version');
    assert.equal(value.source.fixed_revision, undefined);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
