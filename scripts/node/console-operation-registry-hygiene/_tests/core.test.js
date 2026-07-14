const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectConsoleOperationRegistryInventory,
  evaluateConsoleOperationRegistryHygiene,
  main,
  runCompiledAssemblyChecks,
} = require('../core.js');

const FIXTURE_PATH = path.join(__dirname, 'fixtures', 'compiled-healthy.json');

const LOCALE_SOURCE = {
  console: {
    operations: {
      core_authenticated: { label: '已登录' },
      applications: {
        view: { label: '查看应用', description: '查看当前空间应用' },
        run: { label: '运行应用', description: '运行当前空间应用' },
      },
    },
    resources: {
      applications: {
        label: '应用',
        description: '当前空间应用',
        actions: {
          view: { label: '查看', description: '查看应用记录' },
        },
      },
    },
  },
};

function writeJson(repoRoot, relativePath, value) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function writeText(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function readFixture() {
  return JSON.parse(fs.readFileSync(FIXTURE_PATH, 'utf8'));
}

function createFixtureRepo({ drift = false } = {}) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-console-operation-'));
  const baseline = readFixture();
  const current = structuredClone(baseline);

  if (drift) {
    current.operations = current.operations.filter(
      (operation) => operation.operation_id !== 'applications.view'
    );
    current.route_assembly = current.route_assembly.filter(
      (binding) => ![
        'applications.view',
        'applications.run',
      ].includes(binding.ownership.operation_id)
    );
    current.operations.push({
      operation_id: 'applications.publish',
      owner: {
        kind: 'core',
        owner_id: 'boot-core',
        version: 'test',
      },
      lifecycle: 'active',
      policy_group: { Other: 'other.applications' },
      label_ref: 'console.operations.applications.publish.label',
      description_ref: 'console.operations.applications.publish.description',
      order: 200,
      routes: [{ method: 'POST', path: '/api/console/applications/:id/publish' }],
      authorization: { kind: 'simple' },
    });
    current.route_assembly.push({
      route: { method: 'POST', path: '/api/console/applications/:application_id/publish' },
      ownership: { kind: 'console_operation', operation_id: 'applications.publish' },
    });
    current.migration = {
      unknown_permissions: ['applications.publish.all'],
      authorization_delta: [{
        role_id: 'editor',
        operation_id: 'applications.view',
        before: { scope: 'own' },
        after: { scope: 'scope_all' },
      }],
      rollback_verified: false,
    };
  }

  writeJson(repoRoot, 'tmp/current.json', current);
  writeJson(repoRoot, 'tmp/baseline.json', baseline);
  writeJson(repoRoot, 'web/locales/zh_Hans.json', LOCALE_SOURCE);
  writeJson(repoRoot, 'web/locales/en_US.json', LOCALE_SOURCE);
  writeText(
    repoRoot,
    'api/apps/api-server/src/middleware/require_settings_feature_permission.rs',
    drift
      ? 'registry.access_for_console_route(method, path);\nlet fallback = legacy_permission_code;\nnext.run(request).await;\n'
      : 'registry.access_for_console_route(method, path);\n'
  );
  writeText(
    repoRoot,
    'api/apps/api-server/src/lib.rs',
    'route_assembly.into_router();\n'
  );
  writeText(
    repoRoot,
    'web/app/src/features/settings/components/RolePermissionPanel.tsx',
    drift
      ? 'const RESOURCE_MAP = {};\nconst category = "基础通用";\nconst registry = legacy_permission_code;\n'
      : 'export function RolePermissionPanel() { return <div>console policy catalog</div>; }\n'
  );

  return repoRoot;
}

function passingCompiledChecks() {
  return {
    status: 0,
    authoritative: true,
    commands: [
      {
        label: 'migrated-assembly-owners',
        status: 'passed',
        exitCode: 0,
        passedCount: 1,
        failedCount: 0,
      },
      {
        label: 'console-route-assembly',
        status: 'passed',
        exitCode: 0,
        passedCount: 3,
        failedCount: 0,
      },
    ],
  };
}

test('healthy compiled fixture settles route, locale, migration, and diff checks', () => {
  const repoRoot = createFixtureRepo();
  const inventory = collectConsoleOperationRegistryInventory({
    repoRoot,
    compiledInventoryPath: 'tmp/current.json',
    baselineInventoryPath: 'tmp/baseline.json',
    localeDir: 'web/locales',
  });
  const report = evaluateConsoleOperationRegistryHygiene({
    repoRoot,
    inventory,
    compiledChecks: passingCompiledChecks(),
  });

  assert.equal(report.summary.errors, 0);
  assert.equal(report.summary.warnings, 0);
  assert.deepEqual(report.diff.missing, []);
  assert.deepEqual(report.diff.expansion, []);
  assert.equal(report.compiled_checks.authoritative, true);
});

test('drift fixture reports missing coverage, permission expansion, migration delta, locale gaps, and source warnings', () => {
  const repoRoot = createFixtureRepo({ drift: true });
  const inventory = collectConsoleOperationRegistryInventory({
    repoRoot,
    compiledInventoryPath: 'tmp/current.json',
    baselineInventoryPath: 'tmp/baseline.json',
    localeDir: 'web/locales',
  });
  const report = evaluateConsoleOperationRegistryHygiene({
    repoRoot,
    inventory,
    compiledChecks: passingCompiledChecks(),
  });
  const rules = report.findings.map((finding) => finding.rule);

  assert.ok(report.summary.errors > 0);
  assert.ok(report.diff.missing.some((item) => item.kind === 'operation'));
  assert.ok(report.diff.expansion.some((item) => item.key === 'applications.publish'));
  assert.ok(rules.includes('compiled-inventory-route-missing'));
  assert.ok(rules.includes('permission-expansion-detected'));
  assert.ok(rules.includes('migration-authorization-delta'));
  assert.ok(rules.includes('locale-ref-missing'));
  assert.ok(rules.includes('frontend-role-permission-legacy-map'));
  assert.ok(rules.includes('runtime-legacy-permission-fallback'));
  assert.equal(
    report.findings.find((finding) => finding.rule === 'frontend-role-permission-legacy-map').severity,
    'warning'
  );
});

test('main writes stable JSON and Markdown reports and returns a failure exit code', async () => {
  const repoRoot = createFixtureRepo({ drift: true });
  const stdout = [];
  const stderr = [];

  const status = await main([
    '--compiled-inventory', 'tmp/current.json',
    '--baseline-inventory', 'tmp/baseline.json',
    '--locale-dir', 'web/locales',
  ], {
    repoRoot,
    runCompiledChecksImpl: passingCompiledChecks,
    writeStdout(text) { stdout.push(text); },
    writeStderr(text) { stderr.push(text); },
  });

  assert.equal(status, 1);
  assert.match(stdout.join(''), /console-operation-registry-hygiene\.json/u);
  assert.match(stdout.join(''), /console-operation-registry-hygiene\.md/u);
  assert.match(stderr.join(''), /permission-expansion-detected/u);

  const reportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'console-operation-registry-hygiene.json'
  );
  const markdownPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'console-operation-registry-hygiene.md'
  );
  assert.equal(fs.existsSync(reportPath), true);
  assert.equal(fs.existsSync(markdownPath), true);
  assert.match(fs.readFileSync(markdownPath, 'utf8'), /Permission Expansion Diff/u);
});

test('compiled checks run serial cargo targets and fail when a target fails or executes no test', () => {
  const calls = [];
  const result = runCompiledAssemblyChecks({
    repoRoot: '/repo-root',
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return calls.length === 1
        ? {
          status: 0,
          stdout: 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n',
          stderr: '',
        }
        : {
          status: 1,
          stdout: 'test result: FAILED. 0 passed; 1 failed;\n',
          stderr: 'compiled route coverage failed\n',
        };
    },
  });

  assert.equal(result.status, 1);
  assert.equal(result.authoritative, true);
  assert.equal(calls.length, 2);
  assert.match(calls[0].args.join(' '), /migrated_assembly_contains_every_console_router_owner_assembly/u);
  assert.match(calls[1].args.join(' '), /console_route_assembly/u);
  assert.equal(calls[0].options.cwd, path.join('/repo-root', 'api'));
});
