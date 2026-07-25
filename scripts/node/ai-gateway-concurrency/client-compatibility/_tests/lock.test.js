'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { CLIENT_NAMES, loadLock, validateLock } = require('../lock');
const { resolveClients } = require('../resolver');

test('D7-AC-001: committed lock pins exactly three clients, adapters, protocols, and official plugins', () => {
  const lock = loadLock();
  assert.deepEqual(Object.keys(lock.clients).sort(), [...CLIENT_NAMES].sort());
  assert.equal(lock.clients.claude.binding_env, 'CLAUDE_CODE_EXECUTABLE');
  assert.equal(lock.clients.codex.binding_env, 'CODEX_PATH');
  assert.deepEqual(lock.clients.opencode.adapter_args, ['acp']);
  assert.equal(lock.clients.claude.gateway_protocol, 'anthropic-messages');
  assert.equal(lock.clients.codex.gateway_protocol, 'openai-responses');
  assert.equal(lock.clients.opencode.gateway_protocol, 'openai-chat-completions');
  assert.match(lock.official_plugins.revision, /^[a-f0-9]{40}$/u);
  assert.doesNotMatch(JSON.stringify(lock), /\/home\//u);
});

test('D7-AC-001 controlled negatives reject rolling versions, host paths, and missing real-client binding', () => {
  const base = loadLock();
  const rolling = structuredClone(base);
  rolling.packages.codex.version = 'latest';
  assert.throws(() => validateLock(rolling), /version must be exact/u);
  const hostPath = structuredClone(base);
  hostPath.clients.opencode.executable = '/home/user/opencode';
  assert.throws(() => validateLock(hostPath), /runtime-relative/u);
  const unbound = structuredClone(base);
  unbound.clients.codex.binding_env = null;
  assert.throws(() => validateLock(unbound), /bind its real executable/u);
  const missingPlatform = structuredClone(base);
  missingPlatform.clients.claude.platform_package = 'missing';
  assert.throws(() => validateLock(missingPlatform), /platform package is not pinned/u);
});

test('D7-AC-001: resolver records real executable and adapter digests under one runtime root', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'client-compatibility-runtime-'));
  try {
    const lock = loadLock();
    fs.writeFileSync(path.join(root, 'package-lock.json'), JSON.stringify({ packages: Object.fromEntries(
      Object.values(lock.packages).map((spec) => [`node_modules/${spec.name}`, {
        version: spec.version, integrity: spec.integrity,
      }]),
    ) }));
    for (const relative of new Set(Object.values(lock.clients).flatMap((client) => [
      client.executable, client.adapter_executable,
    ]))) {
      const executable = path.join(root, relative);
      fs.mkdirSync(path.dirname(executable), { recursive: true });
      fs.writeFileSync(executable, `fixture-${path.basename(executable)}`, { mode: 0o755 });
    }
    const clients = resolveClients(lock, root);
    assert.deepEqual(Object.keys(clients).sort(), [...CLIENT_NAMES].sort());
    for (const client of Object.values(clients)) {
      assert.match(client.executable_sha256, /^[a-f0-9]{64}$/u);
      assert.match(client.adapter_sha256, /^[a-f0-9]{64}$/u);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
