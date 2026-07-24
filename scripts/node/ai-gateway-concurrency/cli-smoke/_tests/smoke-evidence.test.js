'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { runCliSmoke } = require('..');

function result(stdout) {
  return {
    started_at: '2026-07-20T00:00:00.000Z',
    finished_at: '2026-07-20T00:00:00.010Z',
    duration_ms: 10,
    exit_code: 0,
    signal: null,
    timed_out: false,
    stdout: { text: `${JSON.stringify(stdout)}\n`, bytes: 100, overflow: false },
    stderr: { text: '', bytes: 0, overflow: false },
  };
}

function files() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-smoke-evidence-'));
  const executable = (name) => {
    const file = path.join(root, name);
    fs.writeFileSync(file, '#!/bin/sh\n', { mode: 0o700 });
    return file;
  };
  const readyManifest = path.join(root, 'ready.json');
  fs.writeFileSync(readyManifest, JSON.stringify({
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    gateway_base_url: 'http://127.0.0.1:41002',
    targets: {
      openai: {
        application_id: 'openai-app', model: 'fixture-model', api_key: 'sk-openai-secret',
        gateway: { base_url: 'http://127.0.0.1:41002' },
      },
      anthropic: {
        application_id: 'anthropic-app', model: 'fixture-model', api_key: 'sk-anthropic-secret',
        gateway: { base_url: 'http://127.0.0.1:41002' },
      },
    },
  }));
  return {
    root,
    outputRoot: path.join(root, 'evidence'),
    options: {
      readyManifest,
      codexExecutable: executable('codex'),
      claudeExecutable: executable('claude'),
      opencodeExecutable: executable('opencode'),
      codexSourceRoot: root,
      codexSourceIdentity: 'github:openai/codex',
      codexBuildCommand: 'cargo build --release --locked',
      claudePackageName: '@anthropic-ai/claude-code',
      claudePackageVersion: 'fixed-test-version',
      claudePackageIntegrity: 'sha512-fixed-test-integrity',
      claudeInstallCommand: 'npm install --global @anthropic-ai/claude-code@fixed-test-version',
      opencodeSourceRoot: root,
      opencodeSourceIdentity: 'github:configured/opencode',
      opencodeBuildCommand: 'bun run build',
    },
  };
}

function provenanceFixture() {
  const entry = (client, claim) => ({
    client_kind: client,
    provenance_claim: claim,
    executable: { sha256: '0'.repeat(64) },
  });
  return {
    codex: entry('codex', 'source-built-from-fixed-git-commit'),
    claude: entry('claude', 'pinned-package-binary'),
    opencode: entry('opencode', 'source-built-from-fixed-git-commit'),
  };
}

// Root #1377 AC-006/007/008: fake spawn binds construction to isolated dirs/env and evidence.
test('smoke writes sanitized evidence and removes both temporary client homes', async () => {
  const fixture = files();
  const calls = [];
  try {
    const summary = await runCliSmoke(fixture.options, {
      outputRoot: fixture.outputRoot,
      parentEnv: {
        PATH: '/safe/bin',
        HOME: '/real/home',
        EVIL_PARENT_CANARY: 'parent-canary',
        OPENAI_API_KEY: 'real-openai-key',
      },
      collectClientProvenance: provenanceFixture,
      async executeInvocation(invocation, env) {
        calls.push({ invocation, env });
        assert.equal(fs.existsSync(invocation.cwd), true);
        assert.notEqual(env.HOME, '/real/home');
        assert.equal(env.EVIL_PARENT_CANARY, undefined);
        if (path.basename(invocation.executable) === 'codex') {
          return result({
            type: 'item.completed',
            item: { type: 'agent_message', text: '1flowbase gateway sentinel ok sk-openai-secret' },
          });
        }
        if (path.basename(invocation.executable) === 'claude') {
          assert.equal(fs.existsSync(invocation.settingsPath), true);
          return result({ type: 'result', result: '1flowbase gateway sentinel ok sk-anthropic-secret' });
        }
        return result({ type: 'text', part: { text: '1flowbase gateway sentinel ok sk-openai-secret' } });
      },
    });
    assert.equal(summary.status, 'pass');
    assert.equal(calls.length, 3);
    for (const call of calls) assert.equal(fs.existsSync(path.dirname(call.invocation.cwd)), false);

    const combined = ['config-manifest.json', 'codex.json', 'claude.json', 'opencode.json']
      .map((name) => fs.readFileSync(path.join(fixture.outputRoot, name), 'utf8'))
      .join('\n');
    assert.doesNotMatch(combined, /sk-openai-secret|sk-anthropic-secret|parent-canary|real-openai-key/u);
    assert.match(combined, /<redacted-application-key>|<ephemeral-application-key>/u);
    assert.match(combined, /--ignore-user-config/u);
    assert.match(combined, /--bare/u);
    assert.match(combined, /oneflowbase_gateway\/fixture-model/u);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('controlled negative preserves nonzero evidence and fails the sentinel', async () => {
  const fixture = files();
  try {
    await assert.rejects(
      runCliSmoke(fixture.options, {
        outputRoot: fixture.outputRoot,
        parentEnv: { PATH: '/safe/bin' },
        collectClientProvenance: provenanceFixture,
        async executeInvocation() {
          return {
            ...result({ type: 'error', message: 'controlled failure' }),
            exit_code: 2,
          };
        },
      }),
      /codex sentinel exited with 2/u
    );
    const evidence = JSON.parse(fs.readFileSync(path.join(fixture.outputRoot, 'codex.json')));
    assert.equal(evidence.exit_code, 2);
    assert.equal(fs.existsSync(path.join(fixture.outputRoot, 'claude.json')), false);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});
