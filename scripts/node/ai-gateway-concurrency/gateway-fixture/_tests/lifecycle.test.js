'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { createGatewayFixture } = require('..');
const { createPublishedApplication } = require('../bootstrap');
const { persistServiceLogs } = require('../service-logs');

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
      artifactRoot: path.join(root, 'artifacts'),
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
    this.instanceCounts = { openai: 0, anthropic: 0 };
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
      this.instanceCounts[code] += 1;
      return { data: { id: `${code}-instance-${this.instanceCounts[code]}` } };
    }
    if (pathname === '/api/console/applications') {
      const code = body.name.includes('openai') ? 'openai' : 'anthropic';
      this.applicationCount += 1;
      const ordinal = code === 'openai' ? 1 : this.applicationCount - 1;
      return { data: { id: `${code}-application-${ordinal}` } };
    }
    if (pathname.endsWith('/api-keys')) {
      const application = pathname.split('/').at(-2);
      return { data: { id: `${application}-key-id`, token: `sk-${application}-fixture` } };
    }
    if (pathname.endsWith('/api-publications')) {
      return { data: { id: `${pathname.split('/').at(-2)}-publication` } };
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

function fakeDependencies({ OwnerClient = FakeOwnerClient, persistLogs = persistServiceLogs } = {}) {
  const spawned = [];
  const stopped = [];
  const events = [];
  const ports = [41001, 41002];
  return {
    spawned,
    stopped,
    events,
    dependencies: {
      reserveLoopbackPort: async () => ports.shift(),
      spawnOwned(binary, env) {
        const output = [
          env.API_DATABASE_URL,
          env.BOOTSTRAP_ROOT_PASSWORD,
          env.API_PROVIDER_SECRET_MASTER_KEY,
          'gateway_session=fake',
          'fixture-openai-token',
          'fixture-anthropic-token',
          'sk-application-canary',
        ].filter(Boolean).join(' ');
        const handle = {
          binary,
          env,
          child: { exitCode: null },
          stdout: () => output,
          stderr: () => `stderr ${output}`,
          output: () => output,
        };
        spawned.push(handle);
        return handle;
      },
      waitForHealth: async () => {},
      async stopOwned(handle) {
        if (handle) {
          events.push(`stop:${path.basename(handle.binary)}`);
          stopped.push(handle.binary);
        }
      },
      persistServiceLogs(options) {
        events.push('persist');
        return persistLogs(options);
      },
      removeScratch(target) {
        events.push('rm');
        fs.rmSync(target, { recursive: true, force: true });
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
    assert.equal(fixture.result.targets.openai.application_id, 'openai-application-1');
    assert.equal(fixture.result.targets.anthropic.model, 'gateway-fixture-model');
    assert.equal(fixture.result.pools.anthropic.length, 2);
    assert.deepEqual(
      fixture.result.pools.anthropic.map((target) => target.application_id),
      ['anthropic-application-1', 'anthropic-application-2'],
    );
    assert.deepEqual(
      fixture.result.pools.anthropic.map((target) => target.provider_instance_id),
      ['anthropic-instance-1', 'anthropic-instance-2'],
    );
    assert.equal(new Set(fixture.result.pools.anthropic.map((target) => target.api_key)).size, 2);
    assert.equal(fixture.result.targets.anthropic, fixture.result.pools.anthropic[0]);
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
      'openai-instance-1'
    );
    const anthropicDrafts = FakeOwnerClient.calls.filter(
      (call) => call.kind === 'write'
        && call.pathname.includes('anthropic-application')
        && call.pathname.endsWith('/orchestration/draft')
    );
    assert.deepEqual(anthropicDrafts.map((call) =>
      call.body.document.graph.nodes.find((node) => node.type === 'llm').config.model_provider.source_instance_id
    ), ['anthropic-instance-1', 'anthropic-instance-2']);
    assert.equal(fake.spawned[0].env.OPENAI_API_KEY, '');
    assert.equal(fake.spawned[1].env.ANTHROPIC_API_KEY, '');
    const installRoot = fake.spawned[1].env.API_PROVIDER_INSTALL_ROOT;
    assert.ok(fs.existsSync(path.dirname(installRoot)));

    await fixture.close();
    await fixture.close();
    assert.deepEqual(fake.stopped.map((value) => path.basename(value)), [
      'api-server',
      'plugin-runner',
    ]);
    assert.deepEqual(fake.events, ['persist', 'stop:api-server', 'stop:plugin-runner', 'rm']);
    for (const service of ['api-server', 'plugin-runner']) {
      const log = fs.readFileSync(path.join(files.options.artifactRoot, `service-${service}.log`), 'utf8');
      assert.match(log, /\[REDACTED\]/u);
      assert.doesNotMatch(log, /postgres:\/\/|Fixture-|master|gateway_session=fake|fixture-(?:openai|anthropic)-token|sk-application-canary/u);
    }
    assert.equal(fs.existsSync(path.dirname(installRoot)), false);
  } finally {
    await fixture?.close();
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('publication source binds Generate for Responses, Chat Completions, and Anthropic Messages', async () => {
  FakeOwnerClient.calls = [];
  const client = new FakeOwnerClient('http://127.0.0.1:41002');
  const provider = (providerCode) => ({
    provider_code: providerCode,
    provider_instance_id: `${providerCode}-instance-1`,
    model: 'gateway-fixture-model',
  });

  await createPublishedApplication(client, provider('openai'));
  await createPublishedApplication(client, provider('anthropic'));

  const publications = FakeOwnerClient.calls.filter(
    (call) => call.kind === 'write' && call.pathname.endsWith('/api-publications')
  );
  assert.equal(publications.length, 2);
  const openaiPublication = publications.find(
    (call) => call.pathname.includes('/openai-application-1/')
  );
  const anthropicPublication = publications.find(
    (call) => call.pathname.includes('/anthropic-application-1/')
  );
  const expectedGenerateBindings = {
    generate: { target_node_id: 'node-llm' },
    count_tokens: null,
    compact: {
      responses_compact: null,
      responses_compaction_v2: null,
    },
  };
  const protocolPublications = [
    ['OpenAI Responses', openaiPublication],
    ['OpenAI Chat Completions', openaiPublication],
    ['Anthropic Messages', anthropicPublication],
  ];
  for (const [protocol, publication] of protocolPublications) {
    assert.ok(publication, `${protocol} publication write must exist`);
    assert.deepEqual(
      publication.body.mapping.operation_bindings,
      expectedGenerateBindings,
      `${protocol} must publish the backend Generate operation target`
    );
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
    assert.deepEqual(fake.stopped.map((value) => path.basename(value)), [
      'api-server',
      'plugin-runner',
    ]);
    assert.deepEqual(fake.events, ['persist', 'stop:api-server', 'stop:plugin-runner', 'rm']);
    for (const service of ['api-server', 'plugin-runner']) {
      assert.equal(fs.existsSync(path.join(files.options.artifactRoot, `service-${service}.log`)), true);
    }
    const installRoot = fake.spawned[1].env.API_PROVIDER_INSTALL_ROOT;
    assert.equal(fs.existsSync(path.dirname(installRoot)), false);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('injected service-log write failure still stops both children, removes scratch, and rejects close', async () => {
  const files = fixtureFiles();
  const fake = fakeDependencies({ persistLogs() { throw new Error('controlled log write failure'); } });
  let fixture;
  try {
    fixture = await createGatewayFixture(files.options, fake.dependencies);
    const installRoot = fake.spawned[1].env.API_PROVIDER_INSTALL_ROOT;
    await assert.rejects(fixture.close(), /controlled log write failure/u);
    assert.deepEqual(fake.events, ['persist', 'stop:api-server', 'stop:plugin-runner', 'rm']);
    assert.equal(fs.existsSync(path.dirname(installRoot)), false);
  } finally {
    await fixture?.close().catch(() => {});
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});
