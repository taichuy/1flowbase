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
  ClientTurnError,
  conversationSummary,
  loadApiFileEnvironment,
  loadRunManifest,
  publicError,
  reserveOwnedPort,
  runCountTokensUpgrade,
  transitionLocalUpgrade,
  verifiedSourceCwd,
} = require('../count-tokens-upgrade-runner');
const { redact } = require('../../local-client-acceptance/artifacts');

function observed() {
  return {
    application_id: 'published-deepseek-app',
    after_upgrade_application_id: 'published-deepseek-app',
    publication_id: 'published-deepseek-v1',
    after_upgrade_publication_id: 'published-deepseek-v1',
    before_plugin: { installation_id: 'before-id', plugin_id: 'deepseek@0.1.17', package_sha256: 'sha256:before' },
    after_plugin: { installation_id: 'after-id', plugin_id: 'deepseek@0.1.18', package_sha256: 'sha256:after' },
    baseline_setup: {
      installation_id: 'before-id', publication_id: 'published-deepseek-v1',
      publication_unchanged: true,
    },
    transition_mode: 'existing_local',
    after_package_sha256: 'sha256:after',
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

test('Root #1556 F16 preserves the uploaded-local fallback when after installation is omitted', async () => {
  const archiveSha = 'a'.repeat(64);
  const installationIdValue = '019fbdde-70e8-7862-a2cf-7f9846a4bd1b';
  const beforeInstallationId = '019fbac1-a20d-7830-94f3-74c9040b0169';
  const actions = [];
  let uploads = 0;
  let currentInstallationId = beforeInstallationId;
  const owner = {
    async uploadPackage() {
      uploads += 1;
      return {
        archive_sha256: archiveSha,
        installation: {
          id: installationIdValue,
          provider_code: 'deepseek',
          plugin_version: '2.0.0',
        },
      };
    },
    async write(pathname, method, body) {
      actions.push({ pathname, method, body });
      currentInstallationId = body.installation_id;
    },
    async read() {
      return { data: { entries: [{
        provider_code: 'deepseek',
        current_installation_id: currentInstallationId,
        current_version: currentInstallationId === installationIdValue ? '2.0.0' : '1.0.0',
        current_local_artifact: { local_checksum: currentInstallationId === installationIdValue
          ? archiveSha : 'before-checksum' },
      }] } };
    },
  };

  const transition = await transitionLocalUpgrade(owner, {
    afterInstallationId: null,
    afterPackage: '/local/deepseek.1flowbasepkg',
  }, archiveSha);
  assert.equal(transition.transitionMode, 'uploaded_local');
  assert.equal(transition.afterPlugin.installation_id, installationIdValue);
  assert.equal(uploads, 1);
  assert.deepEqual(actions, [
    {
      pathname: '/api/console/plugins/families/deepseek/switch-version',
      method: 'POST',
      body: { installation_id: installationIdValue },
    },
  ]);
});

test('Root #1556 F21 rejects a switch-version response that leaves another installation current', async () => {
  const target = '019fbdde-70e8-7862-a2cf-7f9846a4bd1b';
  const current = '019fbac1-a20d-7830-94f3-74c9040b0169';
  const writes = [];
  const owner = {
    async read() {
      return { data: { entries: [{
        provider_code: 'deepseek',
        current_installation_id: current,
        current_version: '1.0.0',
        current_local_artifact: { local_checksum: 'before-checksum' },
      }] } };
    },
    async write(pathname, method, body) { writes.push({ pathname, method, body }); },
  };

  await assert.rejects(
    transitionLocalUpgrade(owner, {
      afterInstallationId: target,
      afterPackage: '/local/deepseek.1flowbasepkg',
    }, 'a'.repeat(64)),
    new RegExp(`DeepSeek current installation did not become ${target}`, 'u'),
  );
  assert.deepEqual(writes, [{
    pathname: '/api/console/plugins/families/deepseek/switch-version',
    method: 'POST',
    body: { installation_id: target },
  }]);
});

test('Root #1556 F21 already-current shortcut still verifies the expected package checksum', async () => {
  const target = '019fbdde-70e8-7862-a2cf-7f9846a4bd1b';
  let writes = 0;
  const owner = {
    async read() {
      return { data: { entries: [{
        provider_code: 'deepseek',
        current_installation_id: target,
        current_version: '2.0.0',
        current_local_artifact: { local_checksum: 'b'.repeat(64) },
      }] } };
    },
    async write() { writes += 1; },
  };

  await assert.rejects(
    transitionLocalUpgrade(owner, {
      afterInstallationId: target,
      afterPackage: '/local/deepseek.1flowbasepkg',
    }, 'a'.repeat(64)),
    /existing local DeepSeek installation checksum does not match after_package/u,
  );
  assert.equal(writes, 0);
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

test('Root #1556 F15 emits typed bounded details for initial and follow-up turn failures', () => {
  const sessionId = '11111111-1111-4111-8111-111111111111';
  const assistant = {
    type: 'assistant', session_id: sessionId,
    message: { content: [{ type: 'text', text: 'raw-assistant-secret' }] },
  };
  const terminal = {
    type: 'result', session_id: sessionId, is_error: false,
    terminal_reason: 'completed', result: null,
  };
  const cases = [
    { status: 'timed_out', result: { exit_code: 0, timed_out: true, events: [assistant, terminal] } },
    { status: 'nonzero_exit', result: { exit_code: 7, timed_out: false, events: [assistant, terminal] } },
    { status: 'terminal_missing', result: { exit_code: 0, timed_out: false, events: [assistant] } },
    { status: 'assistant_missing', result: { exit_code: 0, timed_out: false, events: [terminal] } },
  ];

  let sampleError = null;
  for (const turnIndex of [0, 1]) {
    for (const fixture of cases) {
      const result = {
        exit_code: fixture.result.exit_code,
        timed_out: fixture.result.timed_out,
        stdout: fixture.result.events.map(JSON.stringify).join('\n'),
        stderr: 'raw-stderr-secret raw-api-key raw-prompt',
      };
      let error;
      try { conversationSummary(result, { turnIndex, sessionId }); }
      catch (caught) { error = caught; }
      assert.ok(error instanceof ClientTurnError);
      sampleError = error;
      assert.equal(error.code, 'client_turn_failed');
      assert.equal(error.details.stage, turnIndex === 0 ? 'initial' : 'followup');
      assert.equal(error.details.turn_index, turnIndex);
      assert.equal(error.details.transport_status, fixture.status);
      assert.equal(error.details.session_continuity_observed, true);
      assert.deepEqual(Object.keys(error.details).sort(), [
        'assistant_text_count', 'exit_code', 'session_continuity_observed', 'stage',
        'terminal_observed', 'timed_out', 'transport_status', 'turn_index',
      ]);
      assert.doesNotMatch(JSON.stringify(error.details),
        /raw-assistant|raw-stderr|raw-api-key|raw-prompt|11111111/u);
    }
  }
  const artifactError = publicError(sampleError);
  assert.equal(artifactError.code, 'client_turn_failed');
  assert.deepEqual(artifactError.details, sampleError.details);
  assert.doesNotMatch(JSON.stringify(artifactError), /raw-assistant|raw-stderr|raw-prompt|raw-api-key/u);
});

test('Root #1556 F13 structural contract owns frozen services and validated source configuration', () => {
  const source = fs.readFileSync(
    path.resolve(__dirname, '../count-tokens-upgrade-runner.js'),
    'utf8',
  );
  for (const required of [
    'OwnerHttpClient',
    'owner.uploadPackage(archivePath)',
    '/api/console/plugins/families/deepseek/switch-version',
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
  for (const field of ['database_url']) {
    assert.ok(Object.hasOwn(example.environment, field), `run environment omitted ${field}`);
  }
  assert.equal(example.schema_version, '1flowbase.local-count-tokens-upgrade-run/v6');
  assert.equal(example.upgrade.before_installation_id, '019fbac1-a20d-7830-94f3-74c9040b0169');
  assert.equal(example.upgrade.after_installation_id, '019fbdde-70e8-7862-a2cf-7f9846a4bd1b');
  assert.equal(example.environment.owner_username, 'ONEFLOWBASE_OWNER_USERNAME');
  assert.equal(example.environment.owner_password, 'ONEFLOWBASE_OWNER_PASSWORD');
  assert.equal(Object.hasOwn(example.environment, 'owner_cookie'), false);
  assert.equal(Object.hasOwn(example.environment, 'owner_csrf'), false);
  assert.equal(Object.hasOwn(example.environment, 'provider_secret_master_key'), false);
  assert.equal(Object.hasOwn(example.environment, 'provider_install_root'), false);
  assert.deepEqual(Object.keys(example.environment).sort(), [
    'application_api_key',
    'application_api_key_id',
    'database_url',
    'owner_password',
    'owner_username',
  ]);
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

test('Root #1556 F16 explicitly rejects legacy v3 through v5 manifests', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'count-tokens-legacy-manifest-'));
  try {
    const manifestPath = path.join(root, 'run.json');
    for (const schemaVersion of [
      '1flowbase.local-count-tokens-upgrade-run/v3',
      '1flowbase.local-count-tokens-upgrade-run/v4',
      '1flowbase.local-count-tokens-upgrade-run/v5',
    ]) {
      fs.writeFileSync(manifestPath, JSON.stringify({ schema_version: schemaVersion }));
      assert.throws(() => loadRunManifest(manifestPath), /schema mismatch/u);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('Root #1556 F13 executable runner loads owned API dotenv without recording secrets', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'count-tokens-upgrade-runner-'));
  try {
    const executable = path.join(root, 'claude');
    const apiServer = path.join(root, 'api-server');
    const pluginRunner = path.join(root, 'plugin-runner');
    const packageManifest = path.join(root, 'package.json');
    const upgradePackage = path.join(root, 'deepseek.1flowbasepkg');
    const beforeInstallationId = '019fbac1-a20d-7830-94f3-74c9040b0169';
    const afterInstallationId = '019fbdde-70e8-7862-a2cf-7f9846a4bd1b';
    const initialInstallationId = '019fb000-0000-7000-8000-000000000001';
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
    const trustedKeysJson = JSON.stringify([{
      key_id: 'dotenv-key',
      algorithm: 'ed25519',
      public_key_pem: 'quoted-json-value',
    }]);
    const apiEnvPath = path.join(apiServerCwd, '.env');
    const apiEnvContent = [
      'API_ENV=production',
      'API_SERVER_ADDR=127.0.0.1:9999',
      'API_DATABASE_URL=postgres://dotenv:dotenv-password@127.0.0.1/wrong',
      'API_PLUGIN_RUNNER_INTERNAL_BASE_URL=http://127.0.0.1:9998',
      'API_COOKIE_NAME=dotenv_cookie_must_not_win',
      'BOOTSTRAP_WORKSPACE_NAME="dotenv-workspace"',
      "BOOTSTRAP_ROOT_EMAIL='dotenv-root@example.test'",
      'BOOTSTRAP_ROOT_PASSWORD=dotenv-root-password',
      'ACCEPTANCE_SCHEMA=1flowbase.local-count-tokens-upgrade-run/v6',
      'PROVIDER_PATH=/opt/1flowbase/providers/deepseek',
      `API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON='${trustedKeysJson}'`,
      '',
    ].join('\n');
    fs.writeFileSync(apiEnvPath, apiEnvContent);
    fs.writeFileSync(mainSourceReceipt, `${mainSourceSha}\n`);
    fs.writeFileSync(manifestPath, JSON.stringify({
      schema_version: '1flowbase.local-count-tokens-upgrade-run/v6',
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
        owner_username: 'OWNER_USERNAME', owner_password: 'OWNER_PASSWORD',
        database_url: 'DATABASE_URL', provider_secret_master_key: 'PROVIDER_MASTER_KEY',
        provider_install_root: 'PROVIDER_INSTALL_ROOT',
      },
      api_cookie_name: 'flowbase_console_session',
      upgrade: {
        before_installation_id: beforeInstallationId,
        after_installation_id: afterInstallationId,
        after_package: upgradePackage,
      },
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
    let currentInstallationId = initialInstallationId;
    let uploadCalls = 0;
    const pluginActions = [];
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
          current_installation_id: currentInstallationId,
          current_version: currentInstallationId === afterInstallationId ? '2.0.0' : '1.0.0',
          current_local_artifact: { local_checksum: currentInstallationId === afterInstallationId
            ? crypto.createHash('sha256').update('after-package').digest('hex')
            : 'before-checksum' },
        }] } };
      }
      async uploadPackage() {
        uploadCalls += 1;
        return {
          archive_sha256: crypto.createHash('sha256').update('after-package').digest('hex'),
          installation: { id: afterInstallationId, provider_code: 'deepseek', plugin_version: '2.0.0' },
        };
      }
      async write(pathname, method, body) {
        pluginActions.push({ pathname, method, body });
        if (pathname.endsWith('/switch-version')) {
          currentInstallationId = body.installation_id;
        }
        return { data: { status: 'succeeded' } };
      }
    }
    const stdout = [
      JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'ok' }] } }),
      JSON.stringify({ type: 'result', is_error: false, terminal_reason: 'completed', result: 'ok' }),
    ].join('\n');
    const spawned = [];
    const lifecycle = [];
    let attachedSession = null;
    const sourceEnv = {
      APP_KEY: 'secret-app-key', APP_KEY_ID: 'key-id',
      OWNER_USERNAME: 'root', OWNER_PASSWORD: 'secret-owner-password',
      DATABASE_URL: 'postgres://owner:1flowbase@127.0.0.1:35432/dev',
      API_PROVIDER_SECRET_MASTER_KEY: 'inherited-master-key-must-not-win',
      API_PROVIDER_INSTALL_ROOT: '/inherited/install/root/must-not-win',
    };
    const disposableRegistry = (onClose = () => {}) => {
      const roots = [];
      return {
        addTempRoot(value) { roots.push(value); return value; },
        async close() {
          for (const value of roots) fs.rmSync(value, { recursive: true, force: true });
          onClose();
          return [];
        },
      };
    };
    const temporarySession = {
      cookie: 'flowbase_console_session=secret-cookie',
      csrfToken: 'secret-csrf',
      async dispose() { lifecycle.push('owner-session-disposed'); },
    };
    const result = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv,
      OwnerHttpClient: class extends Owner {
        attachSession(cookie, csrfToken) { attachedSession = { cookie, csrfToken }; }
      },
      openTemporaryOwnerSession: async (options) => {
        lifecycle.push('owner-session-opened');
        assert.equal(options.apiBaseUrl, 'http://127.0.0.1:41732');
        assert.equal(options.account, 'root');
        assert.equal(options.password, 'secret-owner-password');
        return temporarySession;
      },
      fetchImpl: async () => new Response('{"input_tokens":17}', { status: 200 }),
      reserveLoopbackPort: (() => {
        const ports = [41731, 41732];
        return async () => ports.shift();
      })(),
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      spawnOwned: (binary, env, options) => {
        spawned.push({ binary, env, parentEnv: options.parentEnv, cwd: options.cwd });
        return { child: { binary, exitCode: null } };
      },
      waitForHealth: async () => {},
      stopOwned: async () => { lifecycle.push('process-stopped'); },
      executeTmux: async () => ({ exit_code: 0, timed_out: false, stdout, stderr: '' }),
      registry: disposableRegistry(() => lifecycle.push('resources-closed')),
    });
    assert.equal(result.status, 'pass');
    assert.equal(result.availability, 'available');
    const encoded = JSON.stringify(result);
    assert.doesNotMatch(
      encoded,
      /secret-app-key|secret-owner-password|secret-cookie|secret-csrf|secret-provider-master-key|owner:1flowbase/u,
    );
    assert.doesNotMatch(encoded, /inherited-master-key|inherited\/install/u);
    assert.doesNotMatch(encoded, /dotenv-workspace|dotenv-root@example|dotenv-key|quoted-json-value/u);
    assert.doesNotMatch(encoded, /dotenv-password|dotenv_cookie_must_not_win|127\.0\.0\.1:999[89]/u);
    assert.deepEqual(attachedSession, {
      cookie: 'flowbase_console_session=secret-cookie',
      csrfToken: 'secret-csrf',
    });
    assert.equal(lifecycle[0], 'owner-session-opened');
    assert.ok(lifecycle.indexOf('owner-session-disposed') < lifecycle.indexOf('process-stopped'));
    assert.equal(result.evidence.publication_id, 'publication-1');
    assert.equal(result.evidence.count_tokens.input_tokens, 17);
    assert.equal(result.evidence.runtime.main_source_sha, mainSourceSha);
    assert.equal(result.evidence.runtime.api_server.port, 41732);
    assert.equal(result.evidence.runtime.plugin_runner.port, 41731);
    assert.equal(result.evidence.runtime.api_server_cwd, apiServerCwd);
    assert.equal(result.evidence.plugin_upgrade.transition_mode, 'existing_local');
    assert.equal(result.evidence.plugin_upgrade.baseline_setup.installation_id, beforeInstallationId);
    assert.equal(result.evidence.plugin_upgrade.before.installation_id, beforeInstallationId);
    assert.equal(result.evidence.plugin_upgrade.after.installation_id, afterInstallationId);
    assert.equal(uploadCalls, 0);
    assert.deepEqual(pluginActions.slice(0, 2), [
      {
        pathname: '/api/console/plugins/families/deepseek/switch-version',
        method: 'POST',
        body: { installation_id: beforeInstallationId },
      },
      {
        pathname: '/api/console/plugins/families/deepseek/switch-version',
        method: 'POST',
        body: { installation_id: afterInstallationId },
      },
    ]);
    assert.equal(pluginActions.filter(({ pathname }) => pathname.endsWith('/enable')).length, 0);
    assert.equal(pluginActions.filter(({ pathname }) => pathname.endsWith('/assign')).length, 0);
    assert.deepEqual(result.evidence.runtime.api_env_source, {
      path: apiEnvPath,
      sha256: crypto.createHash('sha256').update(apiEnvContent).digest('hex'),
    });
    assert.equal(spawned.find((row) => row.binary === apiServer).cwd, apiServerCwd);
    assert.notEqual(spawned.find((row) => row.binary === pluginRunner).cwd, apiServerCwd);
    assert.equal(Object.hasOwn(spawned.find((row) => row.binary === apiServer).env,
      'API_PROVIDER_SECRET_MASTER_KEY'), false);
    assert.equal(Object.hasOwn(spawned.find((row) => row.binary === apiServer).env,
      'API_PROVIDER_INSTALL_ROOT'), false);
    assert.equal(Object.hasOwn(spawned.find((row) => row.binary === apiServer).parentEnv,
      'API_PROVIDER_SECRET_MASTER_KEY'), false);
    assert.equal(Object.hasOwn(spawned.find((row) => row.binary === apiServer).parentEnv,
      'API_PROVIDER_INSTALL_ROOT'), false);
    const spawnedApiEnv = spawned.find((row) => row.binary === apiServer).env;
    assert.equal(spawnedApiEnv.BOOTSTRAP_WORKSPACE_NAME, 'dotenv-workspace');
    assert.equal(spawnedApiEnv.BOOTSTRAP_ROOT_EMAIL, 'dotenv-root@example.test');
    assert.equal(spawnedApiEnv.API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON, trustedKeysJson);
    assert.deepEqual(JSON.parse(spawnedApiEnv.API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON),
      JSON.parse(trustedKeysJson));
    assert.equal(spawnedApiEnv.API_ENV, 'development');
    assert.equal(spawnedApiEnv.API_SERVER_ADDR, '127.0.0.1:41732');
    assert.equal(spawnedApiEnv.API_DATABASE_URL, sourceEnv.DATABASE_URL);
    assert.equal(spawnedApiEnv.API_PLUGIN_RUNNER_INTERNAL_BASE_URL, 'http://127.0.0.1:41731');
    assert.equal(spawnedApiEnv.API_COOKIE_NAME, 'flowbase_console_session');
    const spawnedPluginEnv = spawned.find((row) => row.binary === pluginRunner).env;
    assert.equal(Object.hasOwn(spawnedPluginEnv, 'BOOTSTRAP_WORKSPACE_NAME'), false);
    assert.equal(Object.hasOwn(spawnedPluginEnv,
      'API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON'), false);

    const productProviderRoot = path.join(root, 'dotenv-provider-root');
    fs.mkdirSync(productProviderRoot);
    fs.appendFileSync(apiEnvPath, [
      'API_PROVIDER_SECRET_MASTER_KEY=dotenv-provider-master-key',
      `API_PROVIDER_INSTALL_ROOT=${productProviderRoot}`,
      '',
    ].join('\n'));
    const fileEnvironment = loadApiFileEnvironment(apiServerCwd);
    const productProviderApiEnvs = [];
    const productConfigFailure = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv,
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      reserveLoopbackPort: (() => {
        const ports = [41735, 41736];
        return async () => ports.shift();
      })(),
      spawnOwned: (binary, env) => {
        if (binary === apiServer) productProviderApiEnvs.push(env);
        return { child: { exitCode: null } };
      },
      waitForHealth: async () => {},
      openTemporaryOwnerSession: async () => { throw new Error('product dotenv fixture'); },
      stopOwned: async () => {},
      registry: disposableRegistry(),
    });
    assert.equal(productConfigFailure.primary_error.message, 'product dotenv fixture');
    assert.equal(productProviderApiEnvs[0].API_PROVIDER_SECRET_MASTER_KEY,
      'dotenv-provider-master-key');
    assert.equal(productProviderApiEnvs[0].API_PROVIDER_INSTALL_ROOT, productProviderRoot);

    const overrideApiEnvs = [];
    const fileDiagnosticCanary = Object.values(fileEnvironment.values).join('|');
    const loginFailure = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv: {
        ...sourceEnv,
        PROVIDER_MASTER_KEY: 'secret-provider-master-key',
        PROVIDER_INSTALL_ROOT: root,
      },
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      reserveLoopbackPort: (() => {
        const ports = [41733, 41734];
        return async () => ports.shift();
      })(),
      spawnOwned: (binary, env) => {
        if (binary === apiServer) overrideApiEnvs.push(env);
        return { child: { exitCode: null } };
      },
      waitForHealth: async () => {},
      openTemporaryOwnerSession: async () => {
        throw new Error(`owned login failed ${fileDiagnosticCanary}`);
      },
      stopOwned: async () => { throw new Error('owned process cleanup failed'); },
      registry: disposableRegistry(),
    });
    assert.match(loginFailure.primary_error.message, /^owned login failed/u);
    const loginFailureJson = JSON.stringify(loginFailure);
    assert.match(loginFailureJson,
      /dotenv-workspace|dotenv-root@example|dotenv-key|quoted-json-value/u);
    assert.match(loginFailureJson,
      /1flowbase\.local-count-tokens-upgrade-run\/v6|\/opt\/1flowbase\/providers\/deepseek/u);
    assert.equal(loginFailure.primary_error.message.includes(productProviderRoot), true);
    assert.match(loginFailure.primary_error.message,
      /postgres:\/\/<redacted>@127\.0\.0\.1\/wrong/u);
    assert.doesNotMatch(loginFailureJson,
      /dotenv-password|dotenv-root-password|dotenv-provider-master-key/u);
    assert.equal(loginFailure.cleanup.status, 'fail');
    assert.match(loginFailure.cleanup.errors[0].message, /owned process cleanup failed/u);
    assert.equal(overrideApiEnvs[0].API_PROVIDER_SECRET_MASTER_KEY, 'secret-provider-master-key');
    assert.equal(overrideApiEnvs[0].API_PROVIDER_INSTALL_ROOT, root);

    currentInstallationId = initialInstallationId;
    const afterPackageSha256 = crypto.createHash('sha256').update('after-package').digest('hex');
    const initialApiLogs = [
      'INFO anthropic compatible route boundary route="messages" phase="received"',
      'initial-log-secret-canary',
    ].join('\n');
    const providerCountStart = [
      'INFO api_server::provider_runtime: provider runtime operation boundary',
      'operation="count_tokens"',
      'provider_code=deepseek',
      `installation_id=${afterInstallationId}`,
      `package_sha256=${afterPackageSha256}`,
      'phase="start"',
      'status="started"',
    ].join(' ');
    const providerCountEnd = providerCountStart
      .replace('phase="start"', 'phase="end"')
      .replace('status="started"', 'status="succeeded"');
    const followupApiLogs = [
      'INFO anthropic compatible route boundary route="messages_count_tokens" phase="received"',
      providerCountStart,
      providerCountEnd,
    ].join('\n');
    const partialSecret = 'followup-partial-raw-secret';
    let turn = 0;
    const followupFailure = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv,
      OwnerHttpClient: Owner,
      openTemporaryOwnerSession: async () => temporarySession,
      fetchImpl: async () => new Response('{"input_tokens":17}', { status: 200 }),
      reserveLoopbackPort: (() => {
        const ports = [41737, 41738];
        return async () => ports.shift();
      })(),
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      spawnOwned: (binary) => ({
        child: { binary, exitCode: null },
        output: () => binary === apiServer
          ? [initialApiLogs, ...(turn > 1 ? [followupApiLogs] : [])].join('\n')
          : '',
      }),
      waitForHealth: async () => {},
      stopOwned: async () => {},
      executeTmux: async () => {
        turn += 1;
        if (turn === 1) return { exit_code: 0, timed_out: false, stdout, stderr: '' };
        return {
          exit_code: null,
          timed_out: true,
          stdout: [
            JSON.stringify({ type: 'stream_event', event: { type: 'message_start' } }),
            JSON.stringify({
              type: 'stream_event',
              event: { type: 'content_block_delta', delta: { text: partialSecret } },
            }),
          ].join('\n'),
          stderr: `raw stderr ${partialSecret}`,
        };
      },
      registry: disposableRegistry(),
    });
    assert.equal(followupFailure.status, 'fail');
    assert.equal(followupFailure.evidence, null);
    assert.equal(followupFailure.primary_error.details.stage, 'followup');
    assert.equal(followupFailure.primary_error.details.transport_status, 'timed_out');
    assert.equal(followupFailure.diagnostic_receipt.boundaries.selected_installation.installation_id,
      afterInstallationId);
    assert.equal(followupFailure.diagnostic_receipt.boundaries.provider_operation.start.status,
      'observed');
    assert.equal(followupFailure.diagnostic_receipt.boundaries.provider_operation.end.status,
      'observed');
    assert.equal(followupFailure.diagnostic_receipt.boundaries.provider_operation.operation,
      'count_tokens');
    assert.equal(
      followupFailure.diagnostic_receipt.boundaries.anthropic_route_requests.messages.status,
      'not_observed',
    );
    assert.equal(
      followupFailure.diagnostic_receipt.boundaries.anthropic_route_requests.messages_count_tokens
        .count,
      1,
    );
    assert.equal(followupFailure.diagnostic_receipt.deepest_observed_boundary,
      'provider_operation_end');
    assert.doesNotMatch(JSON.stringify(followupFailure),
      /followup-partial-raw-secret|raw stderr|initial-log-secret-canary/u);
    assert.equal(fs.statSync(artifact).mode & 0o777, 0o600);
    assert.doesNotMatch(fs.readFileSync(artifact, 'utf8'),
      /followup-partial-raw-secret|raw stderr/u);

    currentInstallationId = initialInstallationId;
    let rotatedTurn = 0;
    const rotatedSecret = 'rotated-log-secret-canary';
    const prefixMismatch = await runCountTokensUpgrade({ manifest: manifestPath }, {
      sourceEnv,
      OwnerHttpClient: Owner,
      openTemporaryOwnerSession: async () => temporarySession,
      fetchImpl: async () => new Response('{"input_tokens":17}', { status: 200 }),
      reserveLoopbackPort: (() => {
        const ports = [41739, 41740];
        return async () => ports.shift();
      })(),
      git: (_sourceRoot, args) => args.includes('--show-toplevel') ? mainSourceRoot : mainSourceSha,
      spawnOwned: (binary) => ({
        child: { binary, exitCode: null },
        output: () => {
          if (binary !== apiServer) return '';
          if (rotatedTurn <= 1) return initialApiLogs;
          return `${followupApiLogs}\n${rotatedSecret}`;
        },
      }),
      waitForHealth: async () => {},
      stopOwned: async () => {},
      executeTmux: async () => {
        rotatedTurn += 1;
        if (rotatedTurn === 1) return { exit_code: 0, timed_out: false, stdout, stderr: '' };
        return {
          exit_code: null,
          timed_out: true,
          stdout: JSON.stringify({ type: 'stream_event', event: { type: 'message_start' } }),
          stderr: '',
        };
      },
      registry: disposableRegistry(),
    });
    assert.equal(prefixMismatch.diagnostic_receipt.boundaries.assigned_installation.status,
      'observed');
    assert.equal(prefixMismatch.diagnostic_receipt.boundaries.anthropic_route_requests.status,
      'unknown');
    assert.equal(prefixMismatch.diagnostic_receipt.boundaries.owned_api_request.status, 'unknown');
    assert.equal(prefixMismatch.diagnostic_receipt.boundaries.selected_installation.status,
      'unknown');
    assert.equal(prefixMismatch.diagnostic_receipt.boundaries.provider_operation.start.status,
      'unknown');
    assert.doesNotMatch(JSON.stringify(prefixMismatch), /rotated-log-secret-canary/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
