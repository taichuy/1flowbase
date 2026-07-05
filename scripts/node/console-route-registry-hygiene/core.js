const fs = require('node:fs');
const path = require('node:path');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'console-route-registry-hygiene.json';
const MARKDOWN_REPORT_FILE = 'console-route-registry-hygiene.md';
const DEFAULT_MAX_FINDINGS = 200;
const BACKEND_SETTINGS_ROUTES_FILE = path.join(
  'api',
  'crates',
  'access-control',
  'src',
  'settings_routes.rs'
);
const FRONTEND_SETTINGS_SECTIONS_FILE = path.join(
  'web',
  'app',
  'src',
  'features',
  'settings',
  'lib',
  'settings-sections.tsx'
);
const FRONTEND_SETTINGS_API_DIR = path.join(
  'web',
  'app',
  'src',
  'features',
  'settings',
  'api'
);
const SETTINGS_MIDDLEWARE_FILE = path.join(
  'api',
  'apps',
  'api-server',
  'src',
  'middleware',
  'require_settings_route_permission.rs'
);
const API_SERVER_LIB_FILE = path.join(
  'api',
  'apps',
  'api-server',
  'src',
  'lib.rs'
);

const SECTION_API_MODULES = {
  docs: ['api-docs.ts'],
  'api-key-authentication': ['personal-access-tokens.ts'],
  'auth-center': ['auth-center.ts'],
  'system-runtime': ['system-runtime.ts'],
  'host-infrastructure': ['host-infrastructure.ts'],
  'memory-observation': ['host-infrastructure.ts'],
  files: ['file-management.ts'],
  'data-models': ['data-models.ts'],
  'mcp-management': ['mcp-management.ts'],
  'model-providers': ['model-providers.ts', 'plugins.ts'],
  members: ['members.ts', 'roles.ts'],
  roles: ['permissions.ts', 'roles.ts'],
};

const DISALLOWED_FRONTEND_PERMISSION_CODES = [
  'route_page.view.all',
  'user.view.all',
  'role_permission.view.all',
  'plugin_config.view.all',
  'plugin_config.configure.all',
  'system_runtime.view.all',
  'api_reference.view.all',
  'mcp_management.view.all',
  'mcp_management.manage.all',
  'state_model.view.all',
  'state_model.view.own',
  'state_model.manage.all',
  'state_model.manage.own',
  'file_table.view.all',
  'file_table.view.own',
  'file_table.create.all',
];

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function readRequiredFile(repoRoot, relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function parseQuotedStrings(source) {
  return Array.from(source.matchAll(/"([^"]+)"|'([^']+)'/g), (match) => (
    match[1] ?? match[2]
  ));
}

function parseApiScopeConstants(source) {
  const scopesByConst = new Map();
  const pattern = /const\s+([A-Z0-9_]+):\s*&\[\s*SettingsRouteApiScope\s*\]\s*=\s*&\[(.*?)\];/gs;

  for (const match of source.matchAll(pattern)) {
    const scopes = [];
    const scopePattern = /SettingsRouteApiScope\s*\{(.*?)\}/gs;

    for (const scopeMatch of match[2].matchAll(scopePattern)) {
      const body = scopeMatch[1];
      const scopeId = body.match(/scope_id:\s*"([^"]+)"/)?.[1] ?? null;
      const scopePath = body.match(/path:\s*"([^"]+)"/)?.[1] ?? null;

      if (scopeId && scopePath) {
        scopes.push({ scopeId, path: scopePath });
      }
    }

    scopesByConst.set(match[1], scopes);
  }

  return scopesByConst;
}

function extractSettingsRouteSpecsBody(source) {
  const match = source.match(
    /const\s+SETTINGS_ROUTE_SPECS:\s*&\[\s*SettingsRouteSpec\s*\]\s*=\s*&\[(.*?)\];/s
  );

  if (!match) {
    throw new Error('Unable to locate SETTINGS_ROUTE_SPECS in settings_routes.rs');
  }

  return match[1];
}

function parseBackendSettingsRoutes(source) {
  const apiScopes = parseApiScopeConstants(source);
  const specsBody = extractSettingsRouteSpecsBody(source);
  const routes = [];
  const specPattern = /SettingsRouteSpec\s*\{(.*?)\}\s*,/gs;

  for (const match of specsBody.matchAll(specPattern)) {
    const body = match[1];
    const routeId = body.match(/route_id:\s*"([^"]+)"/)?.[1] ?? null;
    const surfaceKey = body.match(/surface_key:\s*"([^"]+)"/)?.[1] ?? null;
    const routePath = body.match(/path:\s*"([^"]+)"/)?.[1] ?? null;
    const labelKey = body.match(/label_key:\s*"([^"]+)"/)?.[1] ?? null;
    const visibilityPermissionCode =
      body.match(/visibility_permission_code:\s*"([^"]+)"/)?.[1] ?? null;
    const apiScopesConst = body.match(/api_scopes:\s*([A-Z0-9_]+)/)?.[1] ?? null;

    if (
      !routeId
      || !surfaceKey
      || !routePath
      || !labelKey
      || !visibilityPermissionCode
    ) {
      throw new Error(
        'Unable to parse a SettingsRouteSpec entry from settings_routes.rs'
      );
    }

    routes.push({
      routeId,
      surfaceKey,
      path: routePath,
      labelKey,
      visibilityPermissionCode,
      apiScopes: apiScopesConst ? (apiScopes.get(apiScopesConst) ?? []) : [],
    });
  }

  return routes;
}

function parseFrontendSettingsSections(source) {
  const match = source.match(
    /export\s+const\s+settingsSectionDefinitions:\s*SettingsSectionDefinition\[\]\s*=\s*\[(.*?)\];/s
  );

  if (!match) {
    throw new Error(
      'Unable to locate settingsSectionDefinitions in settings-sections.tsx'
    );
  }

  const sections = [];
  const objectPattern = /\{(.*?)\}\s*,?/gs;

  for (const entry of match[1].matchAll(objectPattern)) {
    const body = entry[1];
    const key = body.match(/key:\s*'([^']+)'/)?.[1] ?? null;
    const labelKey = body.match(/label_key:\s*'([^']+)'/)?.[1] ?? null;
    const to = body.match(/to:\s*'([^']+)'/)?.[1] ?? null;

    if (!key || !labelKey || !to) {
      continue;
    }

    sections.push({
      key,
      labelKey,
      path: to,
    });
  }

  return sections;
}

function parseApiClientImports(source) {
  const imports = [];
  const pattern = /import\s*\{(.*?)\}\s*from\s*'@1flowbase\/api-client';/gs;

  for (const match of source.matchAll(pattern)) {
    const items = match[1]
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);

    for (const item of items) {
      imports.push(item.replace(/^type\s+/u, '').trim());
    }
  }

  return imports;
}

function parseQueryKeyPrefixes(source) {
  const prefixes = [];
  const pattern = /=\s*\[(.*?)\]\s*as const/gs;

  for (const match of source.matchAll(pattern)) {
    const values = parseQuotedStrings(match[1]);
    if (values.length > 0) {
      prefixes.push(values.join('.'));
    }
  }

  return prefixes;
}

function collectSettingsApiModules(
  repoRoot,
  settingsApiDir = FRONTEND_SETTINGS_API_DIR
) {
  const absoluteDir = path.join(repoRoot, settingsApiDir);
  const modules = new Map();

  if (!fs.existsSync(absoluteDir)) {
    return modules;
  }

  for (const entry of fs.readdirSync(absoluteDir, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith('.ts')) {
      continue;
    }

    const relativePath = normalizePath(path.join(settingsApiDir, entry.name));
    const source = fs.readFileSync(path.join(absoluteDir, entry.name), 'utf8');
    modules.set(entry.name, {
      file: relativePath,
      imports: parseApiClientImports(source),
      queryKeyPrefixes: parseQueryKeyPrefixes(source),
    });
  }

  return modules;
}

function collectSettingsMiddlewareState(
  repoRoot,
  middlewarePath = SETTINGS_MIDDLEWARE_FILE,
  apiServerLibPath = API_SERVER_LIB_FILE
) {
  const middlewareSource = readRequiredFile(repoRoot, middlewarePath);
  const apiServerLibSource = readRequiredFile(repoRoot, apiServerLibPath);

  return {
    middlewarePath,
    apiServerLibPath,
    helperImported: /settings_route_permissions_for_console_request/u.test(
      middlewareSource
    ),
    actorPermissionCheck:
      /\.any\(\s*\|permission_code\|\s*context\.actor\.has_permission\(permission_code\)\s*\)/u.test(
        middlewareSource
      ),
    mountedOnConsoleRouter:
      /require_settings_route_permission/u.test(apiServerLibSource)
      && /from_fn_with_state/u.test(apiServerLibSource),
  };
}

function collectConsoleRouteRegistryInventory({
  repoRoot = getRepoRoot(),
  backendSettingsRoutesPath = BACKEND_SETTINGS_ROUTES_FILE,
  frontendSettingsSectionsPath = FRONTEND_SETTINGS_SECTIONS_FILE,
  frontendSettingsApiDir = FRONTEND_SETTINGS_API_DIR,
  settingsMiddlewarePath = SETTINGS_MIDDLEWARE_FILE,
  apiServerLibPath = API_SERVER_LIB_FILE,
} = {}) {
  const backendSource = readRequiredFile(repoRoot, backendSettingsRoutesPath);
  const frontendSource = readRequiredFile(repoRoot, frontendSettingsSectionsPath);

  return {
    backendSource,
    frontendSource,
    backendRoutes: parseBackendSettingsRoutes(backendSource),
    frontendSections: parseFrontendSettingsSections(frontendSource),
    settingsApiModules: collectSettingsApiModules(repoRoot, frontendSettingsApiDir),
    middlewareState: collectSettingsMiddlewareState(
      repoRoot,
      settingsMiddlewarePath,
      apiServerLibPath
    ),
  };
}

function createFinding({
  rule,
  message,
  sectionKey,
  file,
  severity = 'error',
}) {
  return {
    rule,
    severity,
    sectionKey,
    file,
    message,
  };
}

function evaluateConsoleRouteRegistryHygiene({ inventory }) {
  const findings = [];
  const backendByKey = new Map(
    inventory.backendRoutes.map((route) => [route.surfaceKey, route])
  );
  const frontendByKey = new Map(
    inventory.frontendSections.map((section) => [section.key, section])
  );

  const seenRouteIds = new Set();
  const seenSurfaceKeys = new Set();
  const seenPaths = new Set();
  const seenVisibilityCodes = new Set();

  for (const route of inventory.backendRoutes) {
    if (!route.routeId.startsWith('settings.')) {
      findings.push(createFinding({
        rule: 'settings-route-id-prefix',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message: `settings route_id "${route.routeId}" must start with "settings."`,
      }));
    }

    if (!route.path.startsWith('/settings/')) {
      findings.push(createFinding({
        rule: 'settings-route-path-prefix',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message: `settings frontend path "${route.path}" must start with "/settings/"`,
      }));
    }

    if (seenRouteIds.has(route.routeId)) {
      findings.push(createFinding({
        rule: 'settings-route-id-duplicate',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message: `duplicate settings route_id "${route.routeId}" found in backend registry`,
      }));
    } else {
      seenRouteIds.add(route.routeId);
    }

    if (seenSurfaceKeys.has(route.surfaceKey)) {
      findings.push(createFinding({
        rule: 'settings-route-surface-key-duplicate',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message: `duplicate settings surface_key "${route.surfaceKey}" found in backend registry`,
      }));
    } else {
      seenSurfaceKeys.add(route.surfaceKey);
    }

    if (seenPaths.has(route.path)) {
      findings.push(createFinding({
        rule: 'settings-route-path-duplicate',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message: `duplicate settings frontend path "${route.path}" found in backend registry`,
      }));
    } else {
      seenPaths.add(route.path);
    }

    if (seenVisibilityCodes.has(route.visibilityPermissionCode)) {
      findings.push(createFinding({
        rule: 'settings-route-visibility-code-duplicate',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message:
          `duplicate settings visibility permission "${route.visibilityPermissionCode}" `
          + 'found in backend registry',
      }));
    } else {
      seenVisibilityCodes.add(route.visibilityPermissionCode);
    }

    const expectedVisibilityCode = `settings_route.visible.${route.routeId}`;
    if (route.visibilityPermissionCode !== expectedVisibilityCode) {
      findings.push(createFinding({
        rule: 'settings-route-visibility-code-convention',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message:
          `visibility permission "${route.visibilityPermissionCode}" should match `
          + `"${expectedVisibilityCode}"`,
      }));
    }

    if (route.apiScopes.length === 0) {
      findings.push(createFinding({
        rule: 'settings-route-api-scope-missing',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message:
          `settings route "${route.surfaceKey}" must declare at least one owned API scope`,
      }));
    }
  }

  if (/requiredPermissions\s*:/u.test(inventory.frontendSource)) {
    findings.push(createFinding({
      rule: 'frontend-settings-visibility-mapping-present',
      sectionKey: 'settings-sections',
      file: FRONTEND_SETTINGS_SECTIONS_FILE,
      message:
        'frontend settings registry must not maintain a local requiredPermissions visibility mapping',
    }));
  }

  for (const permissionCode of DISALLOWED_FRONTEND_PERMISSION_CODES) {
    if (inventory.frontendSource.includes(permissionCode)) {
      findings.push(createFinding({
        rule: 'frontend-settings-coarse-permission-reference',
        sectionKey: 'settings-sections',
        file: FRONTEND_SETTINGS_SECTIONS_FILE,
        message:
          `frontend settings registry must not reference coarse permission "${permissionCode}"`,
      }));
    }
  }

  for (const section of inventory.frontendSections) {
    if (!backendByKey.has(section.key)) {
      findings.push(createFinding({
        rule: 'frontend-settings-section-extra',
        sectionKey: section.key,
        file: FRONTEND_SETTINGS_SECTIONS_FILE,
        message:
          `frontend settings section "${section.key}" is not declared in backend settings route specs`,
      }));
    }
  }

  for (const route of inventory.backendRoutes) {
    const section = frontendByKey.get(route.surfaceKey);
    if (!section) {
      findings.push(createFinding({
        rule: 'frontend-settings-section-missing',
        sectionKey: route.surfaceKey,
        file: FRONTEND_SETTINGS_SECTIONS_FILE,
        message:
          `backend settings route "${route.surfaceKey}" is missing from frontend settingsSectionDefinitions`,
      }));
      continue;
    }

    if (section.labelKey !== route.labelKey) {
      findings.push(createFinding({
        rule: 'settings-route-label-key-mismatch',
        sectionKey: route.surfaceKey,
        file: FRONTEND_SETTINGS_SECTIONS_FILE,
        message:
          `frontend label_key "${section.labelKey}" does not match backend "${route.labelKey}"`,
      }));
    }

    if (section.path !== route.path) {
      findings.push(createFinding({
        rule: 'settings-route-path-mismatch',
        sectionKey: route.surfaceKey,
        file: FRONTEND_SETTINGS_SECTIONS_FILE,
        message:
          `frontend path "${section.path}" does not match backend "${route.path}"`,
      }));
    }
  }

  for (const route of inventory.backendRoutes) {
    const expectedModules = SECTION_API_MODULES[route.surfaceKey];
    if (!expectedModules || expectedModules.length === 0) {
      findings.push(createFinding({
        rule: 'settings-section-api-ownership-missing',
        sectionKey: route.surfaceKey,
        file: BACKEND_SETTINGS_ROUTES_FILE,
        message:
          `settings route "${route.surfaceKey}" is missing a tooling-owned API module mapping`,
      }));
      continue;
    }

    for (const moduleName of expectedModules) {
      const moduleInfo = inventory.settingsApiModules.get(moduleName);
      if (!moduleInfo) {
        findings.push(createFinding({
          rule: 'settings-section-api-module-missing',
          sectionKey: route.surfaceKey,
          file: normalizePath(path.join(FRONTEND_SETTINGS_API_DIR, moduleName)),
          message:
            `expected settings API module "${moduleName}" for section "${route.surfaceKey}" is missing`,
        }));
        continue;
      }

      const hasSettingsQueryKey = moduleInfo.queryKeyPrefixes.some((prefix) => (
        prefix === 'settings' || prefix.startsWith('settings.')
      ));
      if (!hasSettingsQueryKey) {
        findings.push(createFinding({
          rule: 'settings-section-api-query-key-prefix',
          sectionKey: route.surfaceKey,
          file: moduleInfo.file,
          message:
            `settings API module "${moduleName}" should use query keys rooted at ['settings', ...]`,
        }));
      }

      const hasConsoleBinding = moduleInfo.imports.some((name) => (
        name.includes('Console')
      ));
      if (!hasConsoleBinding) {
        findings.push(createFinding({
          rule: 'settings-section-api-console-binding',
          sectionKey: route.surfaceKey,
          file: moduleInfo.file,
          message:
            `settings API module "${moduleName}" should bind to console API client functions or DTOs`,
        }));
      }
    }
  }

  if (!inventory.middlewareState.helperImported) {
    findings.push(createFinding({
      rule: 'settings-route-middleware-helper-missing',
      sectionKey: 'console-middleware',
      file: inventory.middlewareState.middlewarePath,
      message:
        'settings route middleware must resolve required permissions via settings_route_permissions_for_console_request',
    }));
  }

  if (!inventory.middlewareState.actorPermissionCheck) {
    findings.push(createFinding({
      rule: 'settings-route-middleware-actor-check-missing',
      sectionKey: 'console-middleware',
      file: inventory.middlewareState.middlewarePath,
      message:
        'settings route middleware must require at least one matching settings route visibility permission',
    }));
  }

  if (!inventory.middlewareState.mountedOnConsoleRouter) {
    findings.push(createFinding({
      rule: 'settings-route-middleware-not-mounted',
      sectionKey: 'console-router',
      file: inventory.middlewareState.apiServerLibPath,
      message:
        'console router must mount require_settings_route_permission so bound settings APIs are gated server-side',
    }));
  }

  return {
    generatedAt: new Date().toISOString(),
    summary: {
      findings: findings.length,
      errors: findings.filter((finding) => finding.severity === 'error').length,
      warnings: findings.filter((finding) => finding.severity === 'warning').length,
    },
    findings,
  };
}

function renderMarkdown(report) {
  const lines = [
    '# Console Route Registry Hygiene',
    '',
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Warnings: ${report.summary.warnings}`,
    '',
  ];

  if (report.findings.length === 0) {
    lines.push('No findings.');
    return `${lines.join('\n')}\n`;
  }

  lines.push('| Severity | Rule | Section | File | Message |');
  lines.push('| --- | --- | --- | --- | --- |');

  for (const finding of report.findings) {
    const message = finding.message
      .replace(/\|/gu, '\\|')
      .replace(/\n/gu, '<br>');
    lines.push(
      `| ${finding.severity} | ${finding.rule} | ${finding.sectionKey} | ${finding.file} | ${message} |`
    );
  }

  return `${lines.join('\n')}\n`;
}

function writeReport({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS }) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });

  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };

  const jsonReportPath = path.join(outputDir, JSON_REPORT_FILE);
  fs.writeFileSync(
    jsonReportPath,
    `${JSON.stringify(reportForDisk, null, 2)}\n`,
    'utf8'
  );

  const markdownReportPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(
    markdownReportPath,
    renderMarkdown(reportForDisk),
    'utf8'
  );

  return {
    jsonReportPath,
    markdownReportPath,
    report: reportForDisk,
  };
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    maxFindings: DEFAULT_MAX_FINDINGS,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--max-findings') {
      options.maxFindings = Number.parseInt(argv[index + 1], 10);
      index += 1;
      continue;
    }

    throw new Error(`Unknown console-route-registry-hygiene option: ${arg}`);
  }

  if (!Number.isInteger(options.maxFindings) || options.maxFindings < 1) {
    throw new Error('--max-findings must be a positive integer');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js console-route-registry-hygiene '
      + '[--max-findings <n>]\n'
      + 'Checks backend settings route specs, frontend settings registration, '
      + 'settings API modules, and console middleware gating.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const inventory = (deps.collectInventoryImpl || collectConsoleRouteRegistryInventory)({
    repoRoot,
  });
  const report = (deps.evaluateImpl || evaluateConsoleRouteRegistryHygiene)({
    inventory,
  });
  const {
    jsonReportPath,
    markdownReportPath,
    report: reportForDisk,
  } = writeReport({
    repoRoot,
    report,
    maxFindings: options.maxFindings,
  });

  writeStdout(
    `[1flowbase-console-route-registry-hygiene] ${report.summary.findings} findings `
      + `(${report.summary.errors} errors, ${report.summary.warnings} warnings). `
      + `Reports: ${normalizePath(path.relative(repoRoot, jsonReportPath))}, `
      + `${normalizePath(path.relative(repoRoot, markdownReportPath))}\n`
  );

  for (const finding of reportForDisk.findings.filter((item) => item.severity === 'error')) {
    writeStderr(
      `[console-route-registry-hygiene:${finding.rule}] ${finding.sectionKey} `
        + `${finding.message}\n`
    );
  }

  return report.summary.errors > 0 ? 1 : 0;
}

module.exports = {
  collectConsoleRouteRegistryInventory,
  evaluateConsoleRouteRegistryHygiene,
  main,
  parseBackendSettingsRoutes,
  parseCliArgs,
  parseFrontendSettingsSections,
  writeReport,
};
