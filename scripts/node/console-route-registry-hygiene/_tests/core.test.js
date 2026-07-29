const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectConsoleRouteRegistryInventory,
  evaluateConsoleRouteRegistryHygiene,
  main,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createFixtureRepo() {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), 'oneflowbase-console-route-registry-')
  );

  writeFile(
    repoRoot,
    'api/crates/access-control/src/settings_features.rs',
    `pub const SYSTEM_DOCS_SETTINGS_FEATURE_ID: &str = "system.docs";
pub const SYSTEM_RUNTIME_SETTINGS_FEATURE_ID: &str = "system.system-runtime";

pub fn core_settings_feature_registrations() -> Vec<SettingsFeatureRegistration> {
  vec![
    SettingsFeatureRegistration {
        feature_id: SYSTEM_DOCS_SETTINGS_FEATURE_ID.to_string(),
        route_id: "settings.docs",
        surface_key: "docs",
        path: "/settings/docs",
        label_key: "auto.api_documentation",
        order: 100,
        api_routes: settings_api_routes(&[("GET", "/api/console/docs/catalog")]),
    },
    SettingsFeatureRegistration {
        feature_id: SYSTEM_RUNTIME_SETTINGS_FEATURE_ID.to_string(),
        route_id: "settings.system-runtime",
        surface_key: "system-runtime",
        path: "/settings/system-runtime",
        label_key: "auto.system_runtime",
        order: 200,
        api_routes: settings_api_routes(&[("GET", "/api/console/system/runtime-profile")]),
    },
  ]
}`
  );

  writeFile(
    repoRoot,
    'web/app/src/features/settings/lib/settings-sections.tsx',
    `export const settingsSectionDefinitions: SettingsSectionDefinition[] = [
  {
    key: 'docs',
    label_key: 'auto.api_documentation',
    to: '/settings/docs',
    requiredPermissions: ['api_reference.view.all']
  },
  {
    key: 'system-runtime',
    label_key: 'auto.runtime_profile',
    to: '/settings/runtime-profile',
    requiredPermissions: ['plugin_config.view.all']
  },
  {
    key: 'ghost',
    label_key: 'auto.ghost',
    to: '/settings/ghost'
  }
];`
  );

  writeFile(
    repoRoot,
    'web/app/src/features/settings/api/api-docs.ts',
    `import { fetchConsoleApiDocsCatalog } from '@1flowbase/api-client';

export const settingsApiDocsCatalogQueryKey = ['settings', 'docs', 'catalog'] as const;

export function fetchSettingsApiDocsCatalog() {
  return fetchConsoleApiDocsCatalog();
}`
  );

  writeFile(
    repoRoot,
    'web/app/src/features/settings/api/system-runtime.ts',
    `import { fetchWorkspaceRuntimeProfile } from '@1flowbase/api-client';

export const settingsSystemRuntimeQueryKey = ['runtime', 'system-runtime'] as const;

export function fetchSettingsSystemRuntimeProfile() {
  return fetchWorkspaceRuntimeProfile();
}`
  );

  writeFile(
    repoRoot,
    'api/apps/api-server/src/middleware/require_settings_feature_permission.rs',
    `fn verify(state: State, context: Context, method: &str, path: &str) -> bool {
  let _rule = state.settings_feature_registry.access_rule(method, path);
  context.actor.has_permission(&permission_code)
}`
  );

  writeFile(
    repoRoot,
    'api/apps/api-server/src/lib.rs',
    `use crate::middleware::require_settings_feature_permission;
use axum::middleware::from_fn_with_state;

fn console_router() {
  let _middleware = from_fn_with_state(state.clone(), require_settings_feature_permission);
}`
  );

  return repoRoot;
}

test('evaluateConsoleRouteRegistryHygiene reports registry and API binding drift', () => {
  const repoRoot = createFixtureRepo();
  const inventory = collectConsoleRouteRegistryInventory({ repoRoot });
  const report = evaluateConsoleRouteRegistryHygiene({ inventory });

  assert.equal(report.summary.errors, 7);
  assert.equal(report.summary.findings, 7);

  const rules = report.findings.map((finding) => finding.rule);
  assert.ok(rules.includes('frontend-settings-visibility-mapping-present'));
  assert.equal(
    rules.filter((rule) => rule === 'frontend-settings-coarse-permission-reference').length,
    2
  );
  assert.ok(rules.includes('frontend-settings-section-extra'));
  assert.ok(rules.includes('settings-route-label-key-mismatch'));
  assert.ok(rules.includes('settings-route-path-mismatch'));
  assert.ok(rules.includes('settings-section-api-query-key-prefix'));
  assert.ok(!rules.includes('settings-section-api-console-binding'));
});

test('main writes json and markdown reports under tmp/test-governance', async () => {
  const repoRoot = createFixtureRepo();
  const stdout = [];
  const stderr = [];

  const status = await main([], {
    repoRoot,
    writeStdout(text) {
      stdout.push(text);
    },
    writeStderr(text) {
      stderr.push(text);
    },
  });

  assert.equal(status, 1);
  assert.match(stdout.join(''), /console-route-registry-hygiene\.json/u);
  assert.match(stdout.join(''), /console-route-registry-hygiene\.md/u);
  assert.match(stderr.join(''), /settings-route-path-mismatch/u);

  const jsonReportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'console-route-registry-hygiene.json'
  );
  const markdownReportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'console-route-registry-hygiene.md'
  );

  assert.equal(fs.existsSync(jsonReportPath), true);
  assert.equal(fs.existsSync(markdownReportPath), true);

  const report = JSON.parse(fs.readFileSync(jsonReportPath, 'utf8'));
  assert.equal(report.summary.errors, 7);
  assert.match(
    fs.readFileSync(markdownReportPath, 'utf8'),
    /Console Route Registry Hygiene/u
  );
});
