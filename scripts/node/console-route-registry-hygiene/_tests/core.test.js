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
    'api/crates/access-control/src/settings_routes.rs',
    `const DOCS_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.docs",
    path: "/api/console/docs/",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::ReadOnly,
}];

const SYSTEM_RUNTIME_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.system",
    path: "/api/console/system/",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::ReadOnly,
}];

const SETTINGS_ROUTE_SPECS: &[SettingsRouteSpec] = &[
    SettingsRouteSpec {
        route_id: "settings.docs",
        surface_key: "docs",
        path: "/settings/docs",
        label_key: "auto.api_documentation",
        order: 100,
        visibility_permission_code: "settings_route.visible.settings.docs",
        legacy_visibility: SettingsRouteLegacyVisibility::Authenticated,
        implied_permissions: &[],
        api_scopes: DOCS_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.system-runtime",
        surface_key: "system-runtime",
        path: "/settings/system-runtime",
        label_key: "auto.system_runtime",
        order: 200,
        visibility_permission_code: "settings_route.visible.settings.system-runtime",
        legacy_visibility: SettingsRouteLegacyVisibility::Authenticated,
        implied_permissions: &[],
        api_scopes: SYSTEM_RUNTIME_API_SCOPES,
    },
];`
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
    'api/apps/api-server/src/middleware/require_settings_route_permission.rs',
    `use access_control::settings_route_permissions_for_console_request;

fn verify(context: Context, required_permissions: Vec<String>) -> bool {
  required_permissions
    .iter()
    .any(|permission_code| context.actor.has_permission(permission_code))
}`
  );

  writeFile(
    repoRoot,
    'api/apps/api-server/src/lib.rs',
    `use crate::middleware::require_settings_route_permission;
use axum::middleware::from_fn_with_state;

fn console_router() {
  let _middleware = from_fn_with_state(state.clone(), require_settings_route_permission);
}`
  );

  return repoRoot;
}

test('evaluateConsoleRouteRegistryHygiene reports registry and API binding drift', () => {
  const repoRoot = createFixtureRepo();
  const inventory = collectConsoleRouteRegistryInventory({ repoRoot });
  const report = evaluateConsoleRouteRegistryHygiene({ inventory });

  assert.equal(report.summary.errors, 8);
  assert.equal(report.summary.findings, 8);

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
  assert.ok(rules.includes('settings-section-api-console-binding'));
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
  assert.equal(report.summary.errors, 8);
  assert.match(
    fs.readFileSync(markdownReportPath, 'utf8'),
    /Console Route Registry Hygiene/u
  );
});
