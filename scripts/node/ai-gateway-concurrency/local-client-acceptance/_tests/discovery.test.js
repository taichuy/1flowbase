'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { discoverClients, findExecutable, probeVersion } = require('../discovery');

test('AC-009 discovers only executable machine binaries and existing config paths', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-client-discovery-'));
  try {
    const bin = path.join(root, 'bin');
    const configs = path.join(root, 'configs');
    fs.mkdirSync(bin);
    fs.mkdirSync(configs);
    for (const client of ['claude', 'codex']) {
      fs.writeFileSync(path.join(bin, client), '#!/bin/sh\n', { mode: 0o700 });
      fs.mkdirSync(path.join(configs, client));
    }
    const result = discoverClients({
      env: { PATH: bin },
      configs: {
        claude: path.join(configs, 'claude'),
        codex: path.join(configs, 'codex'),
        opencode: path.join(configs, 'opencode'),
      },
    });
    assert.equal(result.claude.status, 'ready');
    assert.equal(result.codex.status, 'ready');
    assert.deepEqual(
      { status: result.opencode.status, reason: result.opencode.reason },
      { status: 'skipped', reason: 'binary_not_found' },
    );
    assert.equal(findExecutable('claude', bin), path.join(bin, 'claude'));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-009 version probe reports a failed probe instead of inventing a version', async () => {
  const listeners = {};
  const stream = { on() {} };
  const child = {
    stdout: stream,
    stderr: stream,
    once(event, listener) { listeners[event] = listener; },
    kill() {},
  };
  const pending = probeVersion('/machine/codex', { spawnImpl: () => child });
  listeners.exit(7, null);
  assert.deepEqual(await pending, { status: 'failed', version: null, reason: 'version_probe_exit_7' });
});
