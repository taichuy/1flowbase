'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  buildCountTokensUpgradeEvidence,
  loadCountTokensUpgradeFixture,
} = require('../count-tokens-upgrade');
const {
  APPLICATION_ID,
  reserveOwnedPort,
  runCountTokensUpgrade,
  verifiedSourceCwd,
} = require('../count-tokens-upgrade-runner');
const { redact } = require('../../local-client-acceptance/artifacts');

function observed() {
  return {
    application_id: 'published-deepseek-app',
    after_upgrade_application_id: 'published-deepseek-app',
    publication_id: 'published-deepseek-v1',
    after_upgrade_publication_id: 'published-deepseek-v1',
    before_plugin: { plugin_id: 'deepseek@0.1.17', package_sha256: 'sha256:before' },
    after_plugin: { plugin_id: 'deepseek@0.1.18', package_sha256: 'sha256:after' },
    republish_events: 0,
    network_installs: 0,
    count_tokens_application_id: 'published-deepseek-app',
    count_tokens: {
      operation: 'count_tokens', input_tokens: 41, method: 'provider_estimate',
      coverage: 'complete', unknown_block_count: 0,
    },
    claude: {
      application_id: 'published-deepseek-app', surface: 'tmux', turns: 2,
      continued_session: true,
    },
    cleanup: { status: 'pass', owned_tmux_servers: 0, owned_processes: 0 },
  };
}

test('Root #1556 P13 freezes CountTokens, conversation, local upgrade, and no-republish evidence', () => {
  const fixture = loadCountTokensUpgradeFixture();
  const evidence = buildCountTokensUpgradeEvidence(fixture, observed());
  assert.equal(evidence.status, 'pass');
  assert.equal(evidence.count_tokens.input_tokens, 41);
  assert.equal(evidence.plugin_upgrade.republish_events, 0);
});

test('Root #1556 P13 controlled negative rejects a publication change during plugin upgrade', () => {
  const value = observed();
  value.after_upgrade_publication_id = 'republished-deepseek-v2';
  assert.throws(
    () => buildCountTokensUpgradeEvidence(loadCountTokensUpgradeFixture(), value),
    /republished the application/u,
  );
});

test('Root #1556 F09 rejects shared service ports and fixes the token-bound application', async () => {
  assert.equal(APPLICATION_ID, '019f5443-5b8e-74b2-90e3-c867dbddd37b');
  const candidates = [3100, 7800, 7801, 41731];
  assert.equal(await reserveOwnedPort(async () => candidates.shift()), 41731);
});

test('Root #1556 F10 rejects scratch cwd and verifies the exact source HEAD', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'count-tokens-source-cwd-'));
  try {
    const apiCwd = path.join(root, 'api/apps/api-server');
    const scratch = path.join(root, 'scratch');
    fs.mkdirSync(apiCwd, { recursive: true });
    fs.mkdirSync(scratch);
    const sha = 'b'.repeat(40);
    const git = (_sourceRoot, args) => args.includes('--show-toplevel') ? root : sha;
    assert.throws(
      () => verifiedSourceCwd({ main_source_root: root, api_server_cwd: scratch }, sha, { git }),
      /must be main_source_root\/api\/apps\/api-server/u,
    );
    assert.deepEqual(
      verifiedSourceCwd({ main_source_root: root, api_server_cwd: apiCwd }, sha, { git }),
      { sourceRoot: root, apiServerCwd: apiCwd },
    );
    assert.throws(
      () => verifiedSourceCwd(
        { main_source_root: root, api_server_cwd: apiCwd },
        'c'.repeat(40),
        { git },
      ),
      /HEAD does not match/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('Root #1556 F10 startup diagnostics use the artifact secret redactor', () => {
  const safe = redact({
    primary_error: {
      diagnostic: {
        service: 'api-server',
        stdout: { text: 'db=postgres://owner:db-secret@127.0.0.1/dev', truncated_bytes: 0 },
        stderr: { text: 'key=provider-master-secret', truncated_bytes: 0 },
      },
    },
  }, ['postgres://owner:db-secret@127.0.0.1/dev', 'provider-master-secret']);
  assert.doesNotMatch(JSON.stringify(safe), /db-secret|provider-master-secret/u);
  assert.equal(safe.primary_error.diagnostic.service, 'api-server');
});

test('Root #1556 F09 structural contract owns frozen services and preserves both failure channels', () => {
  const source = fs.readFileSync(
    path.resolve(__dirname, '../count-tokens-upgrade-runner.js'),
    'utf8',
  );
  for (const required of [
    'OwnerHttpClient',
    'owner.uploadPackage(archivePath)',
    '/enable',
    '/assign',
    'pinnedClaudeProvenance',
    'executeTmux',
    'spawnOwned',
    'waitForHealth',
    'stopOwned',
    'await registry.close()',
    'primary_error:',
    'cleanup,',
  ]) {
    assert.ok(source.includes(required), `runner omitted ${required}`);
  }
  assert.match(source, /const sourceEnv = dependencies\.sourceEnv \|\| process\.env;[\s\S]*try \{[\s\S]*finally \{/u);
  assert.match(source, /const safeResult = redact\(result, secrets\)/u);
  assert.doesNotMatch(source, /signIn\(|\/api\/public\/auth\/sign-in/u);
  assert.doesNotMatch(source, /console_base_url|gateway_base_url/u);
  const example = JSON.parse(fs.readFileSync(
    path.resolve(__dirname, '../count-tokens-upgrade.run.example.json'),
    'utf8',
  ));
  for (const field of [
    'main_source_sha', 'main_source_receipt', 'main_source_root', 'api_server_cwd',
    'api_server_binary', 'plugin_runner_binary',
  ]) {
    assert.ok(Object.hasOwn(example, field), `run manifest omitted ${field}`);
  }
  for (const field of ['database_url', 'provider_secret_master_key', 'provider_install_root']) {
    assert.ok(Object.hasOwn(example.environment, field), `run environment omitted ${field}`);
  }
  assert.equal(Object.hasOwn(example, 'endpoints'), false);
  assert.match(source, /api-server cwd must be main_source_root\/api\/apps\/api-server/u);
  assert.match(source, /cwd: manifest\.apiServerCwd/u);
});

test('Root #1556 F09 missing configuration is typed unavailable beside cleanup failure', async () => {
  const result = await runCountTokensUpgrade({}, {
    sourceEnv: {},
    registry: { async close() { return [{ owner: 'tmux', message: 'cleanup fixture' }]; } },
  });
  assert.equal(result.status, 'fail');
  assert.equal(result.availability, 'unavailable');
  assert.equal(result.primary_error.code, 'configuration_unavailable');
  assert.equal(result.cleanup.status, 'fail');
  assert.equal(result.cleanup.errors[0].message, 'cleanup fixture');
});

test('Root #1556 F09 executable runner performs the owned upgrade sequence without recording secrets', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'count-tokens-upgrade-runner-'));
  try {
    const executable = path.join(root, 'claude');
    const apiServer = path.join(root, 'api-server');
    const pluginRunner = path.join(root, 'plugin-runner');
    const packageManifest = path.join(root, 'package.json');
    const upgradePackage = path.join(root, 'deepseek.1flowbasepkg');
    const artifact = path.join(root, 'tmp/test-governance/result.json');
    fs.writeFileSync(executable, '#!/bin/sh\n', { mode: 0o700 });
    fs.writeFileSync(apiServer, '#!/bin/sh\n', { mode: 0o700 });
    fs.writeFileSync(pluginRunner, '#!/bin/sh\n', { mode: 0o700 });
    fs.writeFileSync(packageManifest, JSON.stringify({ name: '@anthropic-ai/claude-code', version: '1.2.3' }));
    fs.writeFileSync(upgradePackage, 'after-package');
    const manifestPath = path.join(root, 'run.json');
    const mainSourceSha = 'a'.repeat(40);
    const mainSourceReceipt = path.join(root, 'main-source-sha.log');
    const mainSourceRoot = path.join(root, 'main-source');
    const apiServerCwd = path.join(mainSourceRoot, 'api/apps/api-server');
    fs.mkdirSync(apiServerCwd, { recursive: true });
    fs.writeFileSync(mainSourceReceipt, `${mainSourceSha}\n`);
    fs.writeFileSync(manifestPath, JSON.stringify({
      schema_version: '1flowbase.local-count-tokens-upgrade-run/v3',
      application_id: APPLICATION_ID,
      main_source_sha: mainSourceSha,
      main_source_receipt: mainSourceReceipt,
      main_source_root: mainSourceRoot,
      api_server_cwd: apiServerCwd,
      api_server_binary: {
        path: apiServer, sha256: crypto.createHash('sha256').update('#!/bin/sh\n').digest('hex'),
        source_sha: mainSourceSha,
      },
      plugin_runner_binary: {
        path: pluginRunner, sha256: crypto.createHash('sha256').update('#!/bin/sh\n').digest('hex'),
        source_sha: mainSourceSha,
      },
      model: 'deepseek-fixture',
      environment: {
        application_api_key: 'APP_KEY', application_api_key_id: 'APP_KEY_ID',
        owner_cookie: 'OWNER_COOKIE', owner_csrf: 'OWNER_CSRF',
        database_url: 'DATABASE_URL', provider_secret_master_key: 'PROVIDER_MASTER_KEY',
        provider_install_root: 'PROVIDER_INSTALL_ROOT',
      },
      api_cookie_name: 'flowbase_console_session',
      upgrade: { after_package: upgradePackage },
      claude: {
        executable,
        provenance: {
          package_manifest: packageManifest,
          package_name: '@anthropic-ai/claude-code',
          package_version: '1.2.3',
          package_integrity: 'sha512-fixture',
          install_command: 'pre-existing pinned local installation',
        },
      },
      artifact,
    }));
    let upgraded = false;
    class Owner {
      attachSession() {}
      async read(pathname) {
        if (pathname.endsWith('/api-keys')) {
          return { data: [{ id: 'key-id', enabled: true, token_prefix: 'secret' }] };
        }
        if (pathname.endsWith('/api-publication')) {
          return { data: { id: 'publication-1', application_id: APPLICATION_ID, active: true, api_enabled: true } };
        }
        return { data: { entries: [{
          provider_code: 'deepseek',
          current_installation_id: upgraded ? 'installation-after' : 'installation-before',
          current_version: upgraded ? '2.0.0' : '1.0.0',
          current_local_artifact: { local_checksum: upgraded
            ? crypto.createHash('sha256').update('after-package').digest('hex')
            : 'before-checksum' },
        }] } };
      }
      async uploadPackage() {
        upgraded = true;
        return {
          archive_sha256: crypto.createHash('sha256').update('after-package').digest('hex'),
          installation: { id: 'installation-after', provider_code: 'deepseek', plugin_version: '2.0.0' },
        };
      }
      async write() { return { data: { status: 'succeeded' } }; }
    }
    const stdout = [
      JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'ok' }] } }),
      JSON.stringify({ type: 'result', is_error: false, terminal_reason: 'completed', result: 'ok' }),
    ].join('\n');
    const spawned = [];
    const result = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv: {
        APP_KEY: 'secret-app-key', APP_KEY_ID: 'key-id',
        OWNER_COOKIE: 'flowbase_console_session=secret-cookie', OWNER_CSRF: 'secret-csrf',
        DATABASE_URL: 'postgres://owner:secret@127.0.0.1:35432/dev',
        PROVIDER_MASTER_KEY: 'secret-provider-master-key',
        PROVIDER_INSTALL_ROOT: root,
      },
      OwnerHttpClient: Owner,
      fetchImpl: async () => new Response('{"input_tokens":17}', { status: 200 }),
      reserveLoopbackPort: (() => {
        const ports = [41731, 41732];
        return async () => ports.shift();
      })(),
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      spawnOwned: (binary, _env, options) => {
        spawned.push({ binary, cwd: options.cwd });
        return { child: { binary, exitCode: null } };
      },
      waitForHealth: async () => {},
      stopOwned: async () => {},
      executeTmux: async () => ({ exit_code: 0, timed_out: false, stdout, stderr: '' }),
      registry: {
        addTempRoot(value) { return value; },
        async close() { fs.rmSync(root, { recursive: true, force: true }); return []; },
      },
    });
    assert.equal(result.status, 'pass');
    const encoded = JSON.stringify(result);
    assert.doesNotMatch(
      encoded,
      /secret-app-key|secret-cookie|secret-csrf|secret-provider-master-key|owner:secret/u,
    );
    assert.equal(result.evidence.publication_id, 'publication-1');
    assert.equal(result.evidence.count_tokens.input_tokens, 17);
    assert.equal(result.evidence.runtime.main_source_sha, mainSourceSha);
    assert.equal(result.evidence.runtime.api_server.port, 41732);
    assert.equal(result.evidence.runtime.plugin_runner.port, 41731);
    assert.equal(result.evidence.runtime.api_server_cwd, apiServerCwd);
    assert.equal(spawned.find((row) => row.binary === apiServer).cwd, apiServerCwd);
    assert.notEqual(spawned.find((row) => row.binary === pluginRunner).cwd, apiServerCwd);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
