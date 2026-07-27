'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { normalizeRunInputs, requireCharacterizeProfile, requireFullSha, singlePackage } = require('../inputs');

test('AC-001 controlled negatives: source SHAs and profile fail closed', () => {
  assert.equal(requireFullSha('a'.repeat(40), 'main SHA'), 'a'.repeat(40));
  for (const invalid of ['a'.repeat(39), 'A'.repeat(40), 'main', 'a'.repeat(41)]) {
    assert.throws(() => requireFullSha(invalid, 'main SHA'), /full lowercase 40-character hex SHA/u);
  }
  assert.equal(requireCharacterizeProfile('characterize'), 'characterize');
  assert.throws(() => requireCharacterizeProfile('regression'), /no approved checked-in budget/u);
});

test('AC-001: package discovery requires exactly one host package per provider', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-package-'));
  assert.throws(() => singlePackage(directory, 'OpenAI'), /exactly one package/u);
  const archive = path.join(directory, 'openai.1flowbasepkg');
  fs.writeFileSync(archive, 'fixture');
  assert.equal(singlePackage(directory, 'OpenAI'), archive);
  fs.writeFileSync(path.join(directory, 'duplicate.1flowbasepkg'), 'fixture');
  assert.throws(() => singlePackage(directory, 'OpenAI'), /exactly one package/u);
});

test('AC-001: run inputs preserve the default PostgreSQL pool owner outside workflow', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-input-'));
  const executable = path.join(root, 'executable');
  fs.writeFileSync(executable, '#!/bin/sh\n');
  fs.chmodSync(executable, 0o755);
  const openai = path.join(root, 'openai');
  const anthropic = path.join(root, 'anthropic');
  const openaiCompatible = path.join(root, 'openai_compatible');
  fs.mkdirSync(openai);
  fs.mkdirSync(anthropic);
  fs.mkdirSync(openaiCompatible);
  fs.writeFileSync(path.join(openai, 'openai.1flowbasepkg'), 'fixture');
  fs.writeFileSync(path.join(anthropic, 'anthropic.1flowbasepkg'), 'fixture');
  fs.writeFileSync(
    path.join(openaiCompatible, 'openai_compatible.1flowbasepkg'),
    'fixture'
  );
  const normalized = normalizeRunInputs({
    mainSourceSha: 'a'.repeat(40),
    officialSourceSha: 'b'.repeat(40),
    profile: 'characterize',
    repoRoot: root,
    databaseUrl: 'postgres://postgres:password@127.0.0.1:5432/fixture',
    apiServerBin: executable,
    pluginRunnerBin: executable,
    openaiPackageDir: openai,
    anthropicPackageDir: anthropic,
    openaiCompatiblePackageDir: openaiCompatible,
    hostTarget: 'x86_64-unknown-linux-gnu',
  });
  assert.equal(Object.hasOwn(normalized, 'databasePoolMaxConnections'), false);
});

test('AC-028/029: paired provider lock is portable and exact', () => {
  const lock = require('../paired-source.lock.json');
  assert.equal(lock.schema_version, '1flowbase.ai-gateway-paired-source/v1');
  assert.equal(lock.official_plugins.repository, 'taichuy/1flowbase-official-plugins');
  assert.match(lock.official_plugins.revision, /^[a-f0-9]{40}$/u);
  assert.equal(
    lock.official_plugins.revision,
    'ee940f10fb3dbee2d50e4ad05206c53b642ccd4a'
  );
  assert.doesNotMatch(JSON.stringify(lock), /\/home\//u);
});
