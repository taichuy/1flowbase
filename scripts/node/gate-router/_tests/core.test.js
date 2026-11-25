const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { BACKEND_SHARDS } = require('../../verify/backend-targets.js');

const {
  buildAdvisoryMessage,
  buildPrVerifyPlan,
  readPrVerifyPlan,
  main,
  readChangedFiles,
  routeChangedFiles,
} = require('../core.js');

const realRepoRoot = path.resolve(__dirname, '../../../..');

test('routeChangedFiles recommends related gates for frontend and backend consistency changes', () => {
  const routes = routeChangedFiles([
    'web/app/src/pages/workspaces/WorkspacePage.tsx',
    'api/crates/orchestration-runtime/src/execution_engine.rs',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
    'repo-backend-static',
    'repo-backend-fmt',
    'repo-backend-check-runtime-storage',
    'backend-consistency',
  ]);
});

test('buildAdvisoryMessage produces an English ASCII non-blocking prompt', () => {
  const routes = routeChangedFiles([
    'web/app/src/i18n/en-US.json',
    'api/crates/storage/durable/postgres/src/repository.rs',
  ]);
  const message = buildAdvisoryMessage({
    mode: 'staged',
    changedFiles: [
      'web/app/src/i18n/en-US.json',
      'api/crates/storage/durable/postgres/src/repository.rs',
    ],
    routes,
  });

  assert.match(message, /\[1flowbase-gate-router\] Advisory only/u);
  assert.match(message, /This hook does not block the commit/u);
  assert.match(message, /repo-tooling/u);
  assert.match(message, /repo-frontend-pr/u);
  assert.match(message, /backend-consistency/u);
  assert.doesNotMatch(message, /[^\x00-\x7F]/u);
});

test('routeChangedFiles does not route frontend state names to backend consistency', () => {
  const routes = routeChangedFiles([
    'web/app/src/features/workspace/useWorkspaceState.ts',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
  ]);
});

test('routeChangedFiles recommends state protocol gates for Anthropic ACP projection changes', () => {
  const routes = routeChangedFiles([
    'api/crates/api/public-gateway/src/routes/application_public_api/compat_sse/protocol_mappers/anthropic_stream.rs',
    'scripts/node/acp-claude-smoke/core.js',
  ]);

  assert(routes.some((route) => route.scope === 'state-protocols'));
  assert(
    routes.some((route) =>
      route.command === 'node scripts/node/verify-state-protocols.js'
    )
  );
  assert(routes.some((route) => route.scope === 'foundation-contract-ai-gateway-fast'));
});

test('AC-001 gate router exposes foundation-specific evidence instead of generic green', () => {
  const routes = routeChangedFiles([
    'api/crates/api/mcp/src/routes/mcp_protocol.rs',
    'web/packages/page-runtime/src/native-react-compiler/source-contract.ts',
  ]);

  assert(routes.some((route) => route.scope === 'foundation-contract-mcp-gateway-fast'));
  assert(routes.some((route) => route.scope === 'foundation-contract-native-react-fast'));
});

test('main reads staged changes from env override and never blocks the commit', async () => {
  let stdout = '';

  const status = await main(['--staged'], {
    env: {
      GATE_ROUTER_CHANGED_FILES: [
        'web/app/src/components/Toolbar.tsx',
        'api/crates/api/applications/src/routes/applications/applications.rs',
      ].join('\n'),
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.match(stdout, /repo-frontend-pr/u);
  assert.match(stdout, /repo-backend-check-apps/u);
  assert.match(stdout, /This hook does not block the commit/u);
});


test('real Git deletions route staged and branch foundation changes while ignoring artifacts', (t) => {
  const fs = require('node:fs');
  const os = require('node:os');
  const path = require('node:path');
  const { execFileSync } = require('node:child_process');
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-router-deletions-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
  git('init', '--quiet');
  git('config', 'user.name', 'Gate fixture');
  git('config', 'user.email', 'fixture@example.invalid');
  const source = 'api/crates/api/mcp/src/routes/mcp_protocol.rs';
  const artifact = 'tmp/deleted-fixture.log';
  for (const file of [source, artifact]) {
    fs.mkdirSync(path.dirname(path.join(repoRoot, file)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, file), 'fixture\n');
  }
  git('add', '.');
  git('commit', '--quiet', '-m', 'baseline');
  const baseRef = git('rev-parse', 'HEAD');
  git('rm', '--quiet', source, artifact);
  const staged = readChangedFiles({ repoRoot, mode: 'staged', baseRef, env: {} });
  assert.deepEqual(staged, [source]);
  git('commit', '--quiet', '-m', 'delete sources');
  const branch = readChangedFiles({ repoRoot, mode: 'branch', baseRef, env: {} });
  assert.deepEqual(branch, staged);
  for (const files of [staged, branch]) {
    const scopes = routeChangedFiles(files).map((route) => route.scope);
    assert.ok(scopes.includes('repo-backend-static'));
    assert.ok(scopes.includes('foundation-contract-mcp-gateway-fast'));
  }
  assert.deepEqual(routeChangedFiles([artifact]), []);
});


test('new contract and runtime owners route to their real CI check shard', () => {
  for (const [directory, shard] of [
    ['interface-runtime', 'core-libs'],
    ['extension-contracts', 'core-libs'],
    ['extension-package-runtime', 'core-libs'],
    ['control-plane-contracts', 'core-libs'],
    ['control-plane-test-support', 'core-libs'],
    ['postgres-test-support', 'core-libs'],
    ['runtime-extension-host', 'runtime-storage'],
    ['runtime-extension-sdk', 'runtime-storage'],
    ['control-plane-postgres-tests', 'apps'],
    ['storage/durable/postgres', 'runtime-storage'],
  ]) {
    const routes = routeChangedFiles([`api/crates/${directory}/src/lib.rs`]);
    assert.deepEqual(routes.filter(({ scope }) => scope.startsWith('repo-backend-check-'))
      .map(({ scope }) => scope), [`repo-backend-check-${shard}`], directory);
  }
});

test('unknown backend owners retain all-shard fallback', () => {
  const routes = routeChangedFiles(['api/crates/future-owner/src/lib.rs']);
  assert.deepEqual(routes.filter(({ scope }) => scope.startsWith('repo-backend-check-'))
    .map(({ scope }) => scope), [
      'repo-backend-check-core-libs', 'repo-backend-check-runtime-storage', 'repo-backend-check-apps',
    ]);
});


test('state protocol owner and wrapper changes route independently, unrelated verification does not', () => {
  for (const file of [
    'scripts/node/verify/state-protocols.js',
    'scripts/node/verify-state-protocols.js',
    'scripts/node/verify-state-protocols/_tests/executed-evidence.test.js',
  ]) {
    assert(routeChangedFiles([file]).some((route) => route.scope === 'state-protocols'), file);
  }
  assert(!routeChangedFiles(['scripts/node/verify/coverage.js']).some((route) => route.scope === 'state-protocols'));
});

test('current Kernel, Gateway and Applications changes select the API apps check owner', () => {
  for (const file of [
    'api/crates/api/kernel/src/contracts/app_state.rs',
    'api/crates/api/applications/src/routes/applications/application_runtime.rs',
    'api/crates/api/public-gateway/src/routes/application_public_api/native.rs',
  ]) {
    assert.deepEqual(routeChangedFiles([file]).filter((route) => route.scope.startsWith('repo-backend-check-'))
      .map((route) => route.scope), ['repo-backend-check-apps']);
  }
});


test('real Git backend-to-docs rename preserves the source owner and foundation pack', (t) => {
  const { execFileSync } = require('node:child_process');
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-router-rename-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
  git('init', '--quiet');
  git('config', 'user.name', 'Gate fixture');
  git('config', 'user.email', 'fixture@example.invalid');
  const source = 'api/crates/api/mcp/src/routes/mcp_protocol.rs';
  const dest = 'docs/archive/protocol.md';
  for (const file of [source, dest]) fs.mkdirSync(path.dirname(path.join(repoRoot, file)), { recursive: true });
  fs.writeFileSync(path.join(repoRoot, source), 'same bytes\n');
  git('add', '.');
  git('commit', '--quiet', '-m', 'baseline');
  const baseRef = git('rev-parse', 'HEAD');
  git('mv', source, dest);
  assert.deepEqual(readChangedFiles({ repoRoot, mode: 'staged', env: {} }), [source, dest]);
  git('commit', '--quiet', '-m', 'move to docs');
  assert.deepEqual(readChangedFiles({ repoRoot, mode: 'branch', baseRef, env: {} }), [source, dest]);
  const plan = readPrVerifyPlan({ repoRoot, baseRef, env: { GATE_ROUTER_CHANGED_FILES: dest } });
  assert.equal(plan.frontend, false);
  assert.ok(plan.backendScopes.includes('repo-backend-static'));
  assert.ok(plan.selectedFoundations.includes('mcp-gateway'));
});

test('CI planner follows the control-plane to test-support back-edge across shards', () => {
  const plan = buildPrVerifyPlan({
    repoRoot: realRepoRoot,
    changedFiles: ['api/crates/control-plane/src/plugin_management/mod.rs'],
  });
  assert.equal(plan.frontend, false);
  assert.equal(plan.tooling, false);
  assert.ok(plan.backendScopes.includes('repo-backend-check-apps'));
  assert.ok(plan.backendScopes.includes('repo-backend-check-core-libs'));
});

test('CI planner selects exactly the shards reachable through the real Cargo.lock graph', () => {
  const read = (file) => fs.readFileSync(path.join(realRepoRoot, file), 'utf8');
  const shards = new Map(BACKEND_SHARDS.flatMap(({ key, packages }) =>
    packages.map((name) => [name, `repo-backend-check-${key}`])));
  const members = /members = \[([^\]]+)\]/u.exec(read('api/Cargo.toml'))[1];
  const paths = new Map([...members.matchAll(/"([^"]+)"/gu)].map(([, directory]) => {
    const name = /^name\s*=\s*"([^"]+)"/mu.exec(read(`api/${directory}/Cargo.toml`))[1];
    return [name, `api/${directory}/src/lib.rs`];
  }));
  const entries = read('api/Cargo.lock').split(/^\[\[package\]\]$/mu).slice(1).map((block) => {
    const fields = Object.fromEntries([...block.matchAll(/^(name|version|source) = ("[^"]+")$/gmu)]
      .map(([, key, value]) => [key, JSON.parse(value)]));
    const deps = /dependencies = \[([^\]]*)\]/u.exec(block)?.[1] ?? '';
    return { ...fields, refs: [...deps.matchAll(/"([^"]+)"/gu)].map(([, ref]) => ref) };
  });
  const local = new Map(entries.filter(({ source }) => !source).map((pkg) => [pkg.name, pkg]));
  const byRef = new Map();
  for (const pkg of entries) {
    for (const ref of [pkg.name, `${pkg.name} ${pkg.version}`,
      ...(pkg.source ? [`${pkg.name} ${pkg.version} (${pkg.source.replace(/#.*$/u, '')})`] : [])]) {
      byRef.set(ref, byRef.has(ref) ? null : pkg);
    }
  }
  const forwardClosure = (start) => {
    const reached = new Set();
    const visit = (pkg) => {
      assert.ok(pkg, 'Cargo.lock references must resolve unambiguously');
      if (reached.has(pkg)) return;
      reached.add(pkg);
      pkg.refs.forEach((ref) => visit(byRef.get(ref)));
    };
    visit(start);
    return reached;
  };
  const closures = new Map([...shards.keys()].map((name) => [name, forwardClosure(local.get(name))]));
  for (const [name, ownShard] of shards) {
    assert.ok(paths.has(name), `${name} must have a real workspace path`);
    const plan = buildPrVerifyPlan({ repoRoot: realRepoRoot, changedFiles: [paths.get(name)] });
    const expected = new Set([ownShard]);
    for (const [dependent, closure] of closures) {
      if (closure.has(local.get(name))) {
        const dependentShard = shards.get(dependent);
        assert.ok(plan.backendScopes.includes(dependentShard), `${name} missed ${dependent} (${dependentShard})`);
        expected.add(dependentShard);
      }
    }
    assert.deepEqual(new Set(plan.backendScopes.filter((scope) => scope.startsWith('repo-backend-check-'))), expected, name);
  }
});

test('CI planner checks all shards when the dependency graph or crate owner is unknown', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-router-no-lock-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const [root, file] of [
    [repoRoot, 'api/apps/api-server/src/main.rs'],
    [realRepoRoot, 'api/crates/unknown-crate/src/lib.rs'],
  ]) {
    assert.deepEqual(buildPrVerifyPlan({ repoRoot: root, changedFiles: [file] }).backendScopes,
      ['repo-backend-static', 'repo-backend-fmt', ...BACKEND_SHARDS.map(({ key }) => `repo-backend-check-${key}`)]);
  }
});

test('CI planner specializes only affected slices', () => {
  const model = buildPrVerifyPlan({ changedFiles: ['api/crates/control-plane/src/model/_tests/prices.rs'] });
  assert.equal(model.tooling, true);
  assert.ok(model.backendScopes.includes('repo-backend-image-llm-vision'));
  assert.ok(model.backendScopes.includes('repo-backend-official-i18n-seed'));
  const web = buildPrVerifyPlan({ changedFiles: ['web/app/src/_tests/routes.test.ts', 'web/app/src/i18n/en_US.json'] });
  assert.deepEqual(web.expectedScopes, ['repo-tooling', 'repo-frontend-pr', 'foundation-contracts']);
  const shared = buildPrVerifyPlan({ changedFiles: ['web/packages/shared-types/src/official-i18n-seed.ts'] });
  assert.equal(shared.tooling, true);
  assert.equal(shared.frontend, true);
  assert.deepEqual(shared.backendScopes, buildPrVerifyPlan().backendScopes);
  assert.deepEqual(buildPrVerifyPlan({ changedFiles: ['docs/guide.md'] }).expectedScopes, ['foundation-contracts']);
});

test('CI planner fails closed for workflow and lockfile changes, unknown root and unavailable diff', () => {
  const all = buildPrVerifyPlan();
  assert.equal(all.backendScopes.length, 7);
  for (const file of ['.github/workflows/verify.yml', 'web/pnpm-lock.yaml', 'web/npm-shrinkwrap.json', 'api/Cargo.lock', 'scripts/node/tooling.js', 'unknown.conf']) {
    assert.deepEqual(buildPrVerifyPlan({ changedFiles: [file] }), all);
  }
  for (const baseRef of ['', '0'.repeat(40), 'a'.repeat(40)]) {
    assert.deepEqual(readPrVerifyPlan({ baseRef, spawnSyncImpl: () => ({ status: 128, stderr: 'missing base' }) }), all);
  }
});
