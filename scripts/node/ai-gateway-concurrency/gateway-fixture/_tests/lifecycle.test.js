'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { createGatewayFixture } = require('..');

function fixtureFiles() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gateway-lifecycle-'));
  const make = (name, mode = 0o600) => {
    const target = path.join(root, name);
    fs.writeFileSync(target, name, { mode });
    return target;
  };
  return {
    root,
    options: {
      databaseUrl: 'postgres://fixture@127.0.0.1/fixture_db',
      apiServerBin: make('api-server', 0o700),
      pluginRunnerBin: make('plugin-runner', 0o700),
      openaiPackage: make('openai.1flowbasepkg'),
      anthropicPackage: make('anthropic.1flowbasepkg'),
      upstreamBaseUrl: 'http://127.0.0.1:9123',
    },
  };
}

class FakeOwnerClient {
  static calls = [];

  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.cookie = null;
    this.csrf = null;
    this.applicationCount = 0;
  }

  async signIn(identifier, password) {
    FakeOwnerClient.calls.push({ kind: 'sign-in', identifier, password });
    this.cookie = 'gateway_session=fake';
    this.csrf = 'fake-csrf';
  }

  async uploadPackage(archivePath) {
    const code = path.basename(archivePath).startsWith('openai.') ? 'openai' : 'anthropic';
    FakeOwnerClient.calls.push({ kind: 'upload', code });
    return {
      installation: { id: `${code}-installation`, provider_code: code },
      archive_sha256: crypto.createHash('sha256').update(fs.readFileSync(archivePath)).digest('hex'),
    };
  }

  async write(pathname, method = 'POST', body) {
    FakeOwnerClient.calls.push({ kind: 'write', pathname, method, body });
    if (pathname === '/api/console/settings/model-providers/instances') {
      const code = body.installation_id.split('-', 1)[0];
      return { data: { id: `${code}-instance` } };
    }
    if (pathname === '/api/console/applications') {
      const code = body.name.includes('openai') ? 'openai' : 'anthropic';
      return { data: { id: `${code}-application` } };
    }
    if (pathname.endsWith('/api-keys')) {
      const code = pathname.includes('openai') ? 'openai' : 'anthropic';
      return { data: { id: `${code}-key-id`, token: `sk-${code}-fixture` } };
    }
    if (pathname.endsWith('/api-publications')) {
      return { data: { id: 'publication-1' } };
    }
    return { data: {} };
  }

  async read(pathname) {
    FakeOwnerClient.calls.push({ kind: 'read', pathname });
    return {
      data: {
        draft: {
          document: {
            graph: {
              nodes: [
                { id: 'node-start', type: 'start', config: {} },
                { id: 'node-llm', type: 'llm', config: {} },
              ],
            },
          },
        },
      },
    };
  }
}

function fakeDependencies({ OwnerClient = FakeOwnerClient } = {}) {
  const spawned = [];
  const stopped = [];
  const ports = [41001, 41002];
  return {
    spawned,
    stopped,
    dependencies: {
      reserveLoopbackPort: async () => ports.shift(),
      spawnOwned(binary, env) {
        const handle = { binary, env, child: { exitCode: null }, output: () => '' };
        spawned.push(handle);
        return handle;
      },
      waitForHealth: async () => {},
      async stopOwned(handle) {
        if (handle) stopped.push(handle.binary);
      },
      OwnerHttpClient: OwnerClient,
    },
  };
}

// Root #1377 AC-001/003/004/005/008: real lifecycle targets are assembled from owner APIs.
test('lifecycle exposes gateway, durable, activity, and active-stream targets then cleans up', async () => {
  FakeOwnerClient.calls = [];
  const files = fixtureFiles();
  const fake = fakeDependencies();
  let fixture;
  try {
    fixture = await createGatewayFixture(files.options, fake.dependencies);
    assert.equal(fixture.result.gateway_base_url, 'http://127.0.0.1:41002');
    assert.equal(fixture.result.targets.openai.application_id, 'openai-application');
    assert.equal(fixture.result.targets.anthropic.model, 'gateway-fixture-model');
    assert.match(fixture.result.targets.openai.durable.cancel_run.url_template, /\{run_id\}\/cancel$/u);
    assert.match(fixture.result.targets.openai.runtime_activity.url, /runtime-activity$/u);
    assert.match(
      fixture.result.targets.anthropic.plugin_runner_active_streams.url,
      /providers\/active-streams$/u
    );

    const instanceWrite = FakeOwnerClient.calls.find(
      (call) => call.kind === 'write' && call.pathname.endsWith('/instances')
    );
    assert.equal(instanceWrite.body.config.base_url, `${files.options.upstreamBaseUrl}/v1`);
    assert.match(instanceWrite.body.config.api_key, /^fixture-(openai|anthropic)-token$/u);
    const draftWrite = FakeOwnerClient.calls.find(
      (call) => call.kind === 'write' && call.pathname.endsWith('/orchestration/draft')
    );
    assert.equal(
      draftWrite.body.document.graph.nodes.find((node) => node.type === 'llm').config.model_provider
        .source_instance_id,
      'openai-instance'
    );
    assert.equal(fake.spawned[0].env.OPENAI_API_KEY, '');
    assert.equal(fake.spawned[1].env.ANTHROPIC_API_KEY, '');
    const installRoot = fake.spawned[1].env.API_PROVIDER_INSTALL_ROOT;
    assert.ok(fs.existsSync(path.dirname(installRoot)));

    await fixture.close();
    await fixture.close();
    assert.deepEqual(fake.stopped.map(path.basename), ['api-server', 'plugin-runner']);
    assert.equal(fs.existsSync(path.dirname(installRoot)), false);
  } finally {
    await fixture?.close();
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('controlled bootstrap failure terminates both owned children and removes scratch files', async () => {
  class FailingOwnerClient extends FakeOwnerClient {
    async signIn() {
      throw new Error('controlled sign-in failure');
    }
  }
  const files = fixtureFiles();
  const fake = fakeDependencies({ OwnerClient: FailingOwnerClient });
  try {
    await assert.rejects(
      createGatewayFixture(files.options, fake.dependencies),
      /controlled sign-in failure/u
    );
    assert.deepEqual(fake.stopped.map(path.basename), ['api-server', 'plugin-runner']);
    const installRoot = fake.spawned[1].env.API_PROVIDER_INSTALL_ROOT;
    assert.equal(fs.existsSync(path.dirname(installRoot)), false);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});
