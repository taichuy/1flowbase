const fs = require('node:fs');
const path = require('node:path');

const {
  DEFAULT_BASELINE_INVENTORY_PATH,
  generateCompiledInventorySnapshot,
} = require('./compiled-inventory-generator.js');
const {
  buildCompiledAssemblyCommands,
  parseCargoTestCounts,
  runCompiledAssemblyChecks,
} = require('./compiled-assembly-checks.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'console-operation-registry-hygiene.json';
const MARKDOWN_REPORT_FILE = 'console-operation-registry-hygiene.md';
const INVENTORY_SCHEMA_VERSION = '1flowbase.console-operation-inventory/v1';
const DEFAULT_MAX_FINDINGS = 200;
const DEFAULT_LOCALES = ['zh_Hans', 'en_US'];
const DEFAULT_LOCALE_DIR = path.join(
  'web',
  'app',
  'src',
  'features',
  'settings',
  'i18n'
);
const DEFAULT_ROLE_PERMISSION_PANEL_FILE = path.join(
  'web',
  'app',
  'src',
  'features',
  'settings',
  'components',
  'RolePermissionPanel.tsx'
);
const DEFAULT_ROUTE_HOST_FILE = path.join(
  'api',
  'apps',
  'api-server',
  'src',
  'lib.rs'
);
const DEFAULT_CONSOLE_MIDDLEWARE_FILE = path.join(
  'api',
  'apps',
  'api-server',
  'src',
  'middleware',
  'require_settings_feature_permission.rs'
);
const DEFAULT_LEGACY_MAPPING_FILE = path.join(
  'api',
  'apps',
  'api-server',
  'src',
  'routes',
  'settings',
  'mcp_management.rs'
);
const SOURCE_SCAN_FILES = {
  rolePermissionPanel: DEFAULT_ROLE_PERMISSION_PANEL_FILE,
  routeHost: DEFAULT_ROUTE_HOST_FILE,
  consoleMiddleware: DEFAULT_CONSOLE_MIDDLEWARE_FILE,
  legacyMapping: DEFAULT_LEGACY_MAPPING_FILE,
};
function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function resolveRepoPath(repoRoot, filePath) {
  return path.isAbsolute(filePath) ? filePath : path.join(repoRoot, filePath);
}

function relativeRepoPath(repoRoot, filePath) {
  return normalizePath(path.relative(repoRoot, filePath));
}

function readJsonFile(repoRoot, filePath) {
  const absolutePath = resolveRepoPath(repoRoot, filePath);
  return {
    file: relativeRepoPath(repoRoot, absolutePath),
    value: JSON.parse(fs.readFileSync(absolutePath, 'utf8')),
  };
}

function readOptionalJsonFile(repoRoot, filePath) {
  const absolutePath = resolveRepoPath(repoRoot, filePath);
  if (!fs.existsSync(absolutePath)) {
    return {
      file: relativeRepoPath(repoRoot, absolutePath),
      value: null,
    };
  }

  return readJsonFile(repoRoot, filePath);
}

function readOptionalTextFile(repoRoot, filePath) {
  const absolutePath = resolveRepoPath(repoRoot, filePath);
  if (!fs.existsSync(absolutePath)) {
    return null;
  }

  return {
    file: relativeRepoPath(repoRoot, absolutePath),
    source: fs.readFileSync(absolutePath, 'utf8'),
  };
}

function routeShape(routePath) {
  return routePath
    .split('/')
    .map((segment) => (
      (segment.startsWith(':') && segment.length > 1)
        || (segment.startsWith('{') && segment.endsWith('}'))
        ? '{}'
        : segment
    ))
    .join('/');
}

function normalizeRoute(route) {
  if (!route || typeof route !== 'object') {
    return null;
  }

  if (typeof route.method !== 'string' || typeof route.path !== 'string') {
    return null;
  }

  return {
    method: route.method.toUpperCase(),
    path: route.path,
  };
}

function routeKey(route) {
  return `${route.method} ${routeShape(route.path)}`;
}

function routeDisplay(route) {
  return `${route.method} ${route.path}`;
}

function normalizeOwner(owner) {
  if (!owner || typeof owner !== 'object') {
    return null;
  }

  const kind = String(owner.kind || '').toLowerCase();
  if (!['core', 'host_extension'].includes(kind)) {
    return null;
  }

  return {
    kind,
    owner_id: owner.owner_id || owner.ownerId || '',
    version: owner.version || '',
  };
}

function normalizeLifecycle(lifecycle) {
  const value = String(lifecycle || '').toLowerCase();
  return value === 'active' || value === 'inactive' ? value : null;
}

function normalizePolicyGroup(group) {
  if (!group || typeof group !== 'object') {
    return null;
  }

  const settingsFeature = group.settings_feature || group.SettingsFeature;
  if (typeof settingsFeature === 'string' && settingsFeature.length > 0) {
    return { kind: 'settings_feature', group_id: settingsFeature };
  }

  const other = group.other || group.Other;
  if (typeof other === 'string' && other.length > 0) {
    return { kind: 'other', group_id: other };
  }

  const kind = String(group.kind || '').toLowerCase();
  const groupId = group.group_id || group.groupId || group.value;
  if (
    (kind === 'settings_feature' || kind === 'other')
    && typeof groupId === 'string'
    && groupId.length > 0
  ) {
    return { kind, group_id: groupId };
  }

  return null;
}

function normalizeAuthorization(authorization) {
  if (!authorization || typeof authorization !== 'object') {
    return null;
  }

  const kind = String(authorization.kind || '').toLowerCase();
  if (kind === 'authenticated' || kind === 'simple') {
    return { kind };
  }

  if (kind === 'resource_action' || kind === 'resourceaction') {
    const resourceCode = authorization.resource_code || authorization.resourceCode;
    const actionCode = authorization.action_code || authorization.actionCode;
    if (typeof resourceCode !== 'string' || typeof actionCode !== 'string') {
      return null;
    }
    return {
      kind: 'resource_action',
      resource_code: resourceCode,
      action_code: actionCode,
    };
  }

  return null;
}

function normalizeOwnership(ownership) {
  if (typeof ownership === 'string') {
    if (ownership.toLowerCase() === 'authenticated') {
      return { kind: 'authenticated' };
    }
    return { kind: 'console_operation', operation_id: ownership };
  }

  if (!ownership || typeof ownership !== 'object') {
    return null;
  }

  const kind = String(ownership.kind || '').toLowerCase();
  if (kind === 'authenticated') {
    return { kind: 'authenticated' };
  }

  if (kind === 'console_operation' || kind === 'consoleoperation') {
    const operationId = ownership.operation_id || ownership.operationId || ownership.value;
    return typeof operationId === 'string' && operationId.length > 0
      ? { kind: 'console_operation', operation_id: operationId }
      : null;
  }

  return null;
}

function normalizeOperation(operation) {
  if (!operation || typeof operation !== 'object') {
    return null;
  }

  const routes = Array.isArray(operation.routes)
    ? operation.routes.map(normalizeRoute)
    : [];

  return {
    operation_id: operation.operation_id || operation.operationId || '',
    authorization_profile_id:
      operation.authorization_profile_id || operation.authorizationProfileId
      || operation.operation_id || operation.operationId || '',
    owner: normalizeOwner(operation.owner),
    lifecycle: normalizeLifecycle(operation.lifecycle),
    policy_group: normalizePolicyGroup(operation.policy_group || operation.policyGroup),
    order: operation.order ?? null,
    routes,
    authorization: normalizeAuthorization(operation.authorization),
  };
}

function normalizeInterface(item) {
  if (!item || typeof item !== 'object') {
    return null;
  }
  return {
    interface_id: item.interface_id || item.interfaceId || '',
    route: normalizeRoute(item.route),
    summary: item.summary || '',
    description: item.description || '',
    authorization_operation_id:
      item.authorization_operation_id || item.authorizationOperationId || null,
  };
}

function normalizeResourceAction(action) {
  if (!action || typeof action !== 'object') {
    return null;
  }

  return {
    action_code: action.action_code || action.actionCode || '',
    label_ref: action.label_ref || action.labelRef || '',
    description_ref: action.description_ref || action.descriptionRef || null,
  };
}

function normalizeResource(resource) {
  if (!resource || typeof resource !== 'object') {
    return null;
  }

  return {
    resource_code: resource.resource_code || resource.resourceCode || '',
    owner: normalizeOwner(resource.owner),
    lifecycle: normalizeLifecycle(resource.lifecycle),
    scope_kind: String(resource.scope_kind || resource.scopeKind || '').toLowerCase() || null,
    identity_field: resource.identity_field || resource.identityField || '',
    scope_field: resource.scope_field || resource.scopeField || null,
    owner_field: resource.owner_field || resource.ownerField || null,
    label_ref: resource.label_ref || resource.labelRef || '',
    description_ref: resource.description_ref || resource.descriptionRef || null,
    actions: Array.isArray(resource.actions)
      ? resource.actions.map(normalizeResourceAction)
      : [],
  };
}

function normalizeCompiledEvidence(raw) {
  const compiled = raw && typeof raw === 'object'
    ? (raw.compiled_inventory || raw.compiledInventory || raw.inventory || raw)
    : {};
  const routeAssembly = raw && typeof raw === 'object'
    ? (raw.route_assembly || raw.routeAssembly || raw.assembly || compiled.route_assembly || [])
    : [];

  return {
    schemaVersion: compiled.schema_version || compiled.schemaVersion || null,
    interfaces: Array.isArray(compiled.interfaces)
      ? compiled.interfaces.map(normalizeInterface)
      : null,
    operations: Array.isArray(compiled.operations)
      ? compiled.operations.map(normalizeOperation)
      : null,
    resources: Array.isArray(compiled.resources)
      ? compiled.resources.map(normalizeResource)
      : null,
    routeAssembly: Array.isArray(routeAssembly)
      ? routeAssembly.map((binding) => ({
        route: normalizeRoute(binding.route || binding),
        ownership: normalizeOwnership(binding.ownership || binding.owner),
      }))
      : null,
    migration: raw?.migration || compiled.migration || null,
  };
}

function collectLocaleSources(repoRoot, localeDir) {
  const absoluteDir = resolveRepoPath(repoRoot, localeDir);
  return Object.fromEntries(DEFAULT_LOCALES.map((locale) => {
    const filePath = path.join(absoluteDir, `${locale}.json`);
    const file = relativeRepoPath(repoRoot, filePath);
    if (!fs.existsSync(filePath)) {
      return [locale, { file, data: null }];
    }

    try {
      return [locale, {
        file,
        data: JSON.parse(fs.readFileSync(filePath, 'utf8')),
      }];
    } catch (error) {
      return [locale, {
        file,
        data: null,
        error: error.message,
      }];
    }
  }));
}

function collectSourceFiles(repoRoot, sourceFiles = SOURCE_SCAN_FILES) {
  return Object.fromEntries(Object.entries(sourceFiles).map(([key, filePath]) => [
    key,
    readOptionalTextFile(repoRoot, filePath),
  ]));
}

function collectConsoleOperationRegistryInventory({
  repoRoot = getRepoRoot(),
  compiledInventoryPath = null,
  baselineInventoryPath = null,
  localeDir = DEFAULT_LOCALE_DIR,
  sourceFiles = SOURCE_SCAN_FILES,
} = {}) {
  const currentFile = compiledInventoryPath
    ? readJsonFile(repoRoot, compiledInventoryPath)
    : null;
  const baselineFile = baselineInventoryPath
    ? readJsonFile(repoRoot, baselineInventoryPath)
    : null;
  const current = currentFile ? normalizeCompiledEvidence(currentFile.value) : null;
  const embeddedBaseline = currentFile?.value?.baseline || currentFile?.value?.base || null;
  const baseline = baselineFile
    ? normalizeCompiledEvidence(baselineFile.value)
    : embeddedBaseline
      ? normalizeCompiledEvidence(embeddedBaseline)
      : null;
  const embeddedLocales = currentFile?.value?.locales || currentFile?.value?.locale_catalogs;
  const localeSources = embeddedLocales && typeof embeddedLocales === 'object'
    ? Object.fromEntries(DEFAULT_LOCALES.map((locale) => [locale, {
      file: `${currentFile.file}#locales.${locale}`,
      data: embeddedLocales[locale] || null,
    }]))
    : collectLocaleSources(repoRoot, localeDir);

  return {
    compiledInventoryPath: currentFile?.file || null,
    baselineInventoryPath: baselineFile?.file || (embeddedBaseline ? 'compiled.baseline' : null),
    current,
    inventory: current,
    baseline,
    localeSources,
    localeDir: normalizePath(localeDir),
    sourceFiles: collectSourceFiles(repoRoot, sourceFiles),
  };
}

function createFinding({
  rule,
  severity = 'error',
  source = 'compiled-assembly',
  message,
  file = null,
  subject = null,
}) {
  return {
    rule,
    severity,
    source,
    file,
    subject,
    message,
  };
}

function emptyDiff() {
  return {
    missing: [],
    expansion: [],
    regrouping: [],
    migration: {
      unknown_permissions: [],
      authorization_delta: [],
    },
  };
}

function compactContract(operation) {
  return {
    lifecycle: operation.lifecycle,
    policy_group: operation.policy_group,
    authorization_profile_id: operation.authorization_profile_id,
    authorization: operation.authorization,
    routes: operation.routes,
  };
}

function stableJson(value) {
  return JSON.stringify(value);
}

function compareCompiledInventories(current, baseline) {
  const diff = emptyDiff();
  if (!current || !baseline || !Array.isArray(current.operations) || !Array.isArray(baseline.operations)) {
    return diff;
  }

  const currentOperations = new Map(current.operations.map((item) => [item.operation_id, item]));
  const baselineOperations = new Map(baseline.operations.map((item) => [item.operation_id, item]));
  const currentResources = new Map((current.resources || []).map((item) => [item.resource_code, item]));
  const baselineResources = new Map((baseline.resources || []).map((item) => [item.resource_code, item]));

  for (const [operationId, operation] of baselineOperations) {
    if (!currentOperations.has(operationId)) {
      diff.missing.push({ kind: 'operation', key: operationId, before: compactContract(operation) });
    }
  }

  for (const [operationId, operation] of currentOperations) {
    if (!baselineOperations.has(operationId)) {
      diff.expansion.push({ kind: 'operation', key: operationId, after: compactContract(operation) });
      continue;
    }

    const baselineOperation = baselineOperations.get(operationId);
    if (stableJson(operation.policy_group) !== stableJson(baselineOperation.policy_group)) {
      diff.regrouping.push({
        kind: 'operation',
        key: operationId,
        before: baselineOperation.policy_group,
        after: operation.policy_group,
      });
    }

    const currentRoutes = new Map(operation.routes.filter(Boolean).map((route) => [routeKey(route), route]));
    const baselineRoutes = new Map(baselineOperation.routes.filter(Boolean).map((route) => [routeKey(route), route]));
    for (const [key, route] of baselineRoutes) {
      if (!currentRoutes.has(key)) {
        diff.missing.push({ kind: 'route', key: `${operationId}: ${routeDisplay(route)}`, before: route });
      }
    }
    for (const [key, route] of currentRoutes) {
      if (!baselineRoutes.has(key)) {
        diff.expansion.push({ kind: 'route', key: `${operationId}: ${routeDisplay(route)}`, after: route });
      }
    }

    if (stableJson(compactContract({ ...operation, policy_group: null }))
      !== stableJson(compactContract({ ...baselineOperation, policy_group: null }))) {
      diff.expansion.push({
        kind: 'operation-contract',
        key: operationId,
        before: compactContract(baselineOperation),
        after: compactContract(operation),
      });
    }
  }

  for (const [resourceCode, resource] of baselineResources) {
    if (!currentResources.has(resourceCode)) {
      diff.missing.push({ kind: 'resource', key: resourceCode, before: resource });
    }
  }
  for (const [resourceCode, resource] of currentResources) {
    if (!baselineResources.has(resourceCode)) {
      diff.expansion.push({ kind: 'resource', key: resourceCode, after: resource });
      continue;
    }

    const baselineResource = baselineResources.get(resourceCode);
    const currentActions = new Map((resource.actions || []).filter(Boolean).map((action) => [action.action_code, action]));
    const baselineActions = new Map((baselineResource.actions || []).filter(Boolean).map((action) => [action.action_code, action]));
    for (const [actionCode, action] of baselineActions) {
      if (!currentActions.has(actionCode)) {
        diff.missing.push({ kind: 'resource-action', key: `${resourceCode}.${actionCode}`, before: action });
      }
    }
    for (const [actionCode, action] of currentActions) {
      if (!baselineActions.has(actionCode)) {
        diff.expansion.push({ kind: 'resource-action', key: `${resourceCode}.${actionCode}`, after: action });
      }
    }
  }

  return diff;
}

function validateCompiledEvidence(evidence, findings, diff) {
  if (!evidence) {
    return;
  }

  if (evidence.schemaVersion !== INVENTORY_SCHEMA_VERSION) {
    findings.push(createFinding({
      rule: 'compiled-inventory-schema-invalid',
      message: `compiled inventory schema must be ${INVENTORY_SCHEMA_VERSION}`,
      subject: evidence.schemaVersion || '(missing)',
    }));
    return;
  }

  if (!Array.isArray(evidence.interfaces) || !Array.isArray(evidence.operations) || !Array.isArray(evidence.resources)) {
    findings.push(createFinding({
      rule: 'compiled-inventory-shape-invalid',
      message: 'compiled inventory must expose interfaces, operations, and resources arrays',
    }));
    return;
  }

  const operationById = new Map();
  const interfaceById = new Map();
  const interfaceByRoute = new Map();
  const resourceByCode = new Map();
  const expectedRoutes = new Map();

  for (const operation of evidence.operations) {
    if (!operation || !operation.operation_id) {
      findings.push(createFinding({
        rule: 'compiled-operation-id-missing',
        message: 'compiled operation is missing operation_id',
      }));
      continue;
    }
    if (operationById.has(operation.operation_id)) {
      findings.push(createFinding({
        rule: 'compiled-operation-duplicate',
        subject: operation.operation_id,
        message: `compiled operation is registered more than once: ${operation.operation_id}`,
      }));
    }
    operationById.set(operation.operation_id, operation);

    if (!operation.owner || !operation.lifecycle || !operation.policy_group || !operation.authorization) {
      findings.push(createFinding({
        rule: 'compiled-operation-contract-incomplete',
        subject: operation.operation_id,
        message: `compiled operation ${operation.operation_id} is missing owner, lifecycle, policy group, or authorization`,
      }));
    }
    if (operation.lifecycle === 'inactive' && operation.routes.length > 0) {
      findings.push(createFinding({
        rule: 'compiled-inactive-owner-route',
        subject: operation.operation_id,
        message: `inactive operation ${operation.operation_id} still owns compiled console routes`,
      }));
    }
    if (operation.routes.length === 0) {
      findings.push(createFinding({
        rule: 'compiled-operation-route-missing',
        subject: operation.operation_id,
        message: `compiled operation ${operation.operation_id} must own at least one console route`,
      }));
    }

    for (const route of operation.routes) {
      if (!route || !route.path || !route.method) {
        findings.push(createFinding({
          rule: 'compiled-operation-route-invalid',
          subject: operation.operation_id,
          message: `compiled operation ${operation.operation_id} contains an invalid route`,
        }));
        continue;
      }
      if (!route.path.startsWith('/api/console')) {
        findings.push(createFinding({
          rule: 'compiled-operation-route-outside-console',
          subject: operation.operation_id,
          message: `compiled route ${routeDisplay(route)} is outside /api/console`,
        }));
      }
      const key = routeKey(route);
      if (expectedRoutes.has(key)) {
        findings.push(createFinding({
          rule: 'compiled-route-ownership-duplicate',
          subject: key,
          message: `compiled route ${routeDisplay(route)} has more than one operation owner`,
        }));
      } else {
        expectedRoutes.set(key, { route, operation });
      }
    }
  }

  for (const item of evidence.interfaces) {
    if (!item?.interface_id || !item.route?.method || !item.route?.path) {
      findings.push(createFinding({
        rule: 'compiled-interface-contract-incomplete',
        message: 'compiled interface is missing interface_id or route metadata',
      }));
      continue;
    }
    if (interfaceById.has(item.interface_id)) {
      findings.push(createFinding({
        rule: 'compiled-interface-id-duplicate',
        subject: item.interface_id,
        message: `compiled interface ID is registered more than once: ${item.interface_id}`,
      }));
    }
    interfaceById.set(item.interface_id, item);
    const key = routeKey(item.route);
    if (interfaceByRoute.has(key)) {
      findings.push(createFinding({
        rule: 'compiled-interface-route-duplicate',
        subject: key,
        message: `compiled route has more than one interface metadata entry: ${routeDisplay(item.route)}`,
      }));
    }
    interfaceByRoute.set(key, item);
    if (!item.summary.trim() || !item.description.trim()
      || !/^[\x00-\x7F]+$/.test(item.summary)
      || !/^[\x00-\x7F]+$/.test(item.description)) {
      findings.push(createFinding({
        rule: 'compiled-interface-metadata-invalid',
        subject: item.interface_id,
        message: `compiled interface ${item.interface_id} must provide non-empty static English ASCII summary and description`,
      }));
    }
    if (item.authorization_operation_id && !operationById.has(item.authorization_operation_id)) {
      findings.push(createFinding({
        rule: 'compiled-interface-operation-missing',
        subject: item.interface_id,
        message: `compiled interface ${item.interface_id} references unknown authorization operation ${item.authorization_operation_id}`,
      }));
    }
  }

  for (const resource of evidence.resources) {
    if (!resource || !resource.resource_code) {
      findings.push(createFinding({
        rule: 'compiled-resource-code-missing',
        message: 'compiled resource is missing resource_code',
      }));
      continue;
    }
    if (resourceByCode.has(resource.resource_code)) {
      findings.push(createFinding({
        rule: 'compiled-resource-duplicate',
        subject: resource.resource_code,
        message: `compiled resource is registered more than once: ${resource.resource_code}`,
      }));
    }
    resourceByCode.set(resource.resource_code, resource);
    if (!resource.owner || !resource.lifecycle || !resource.scope_kind || !resource.identity_field) {
      findings.push(createFinding({
        rule: 'compiled-resource-contract-incomplete',
        subject: resource.resource_code,
        message: `compiled resource ${resource.resource_code} is missing owner, lifecycle, scope, or identity metadata`,
      }));
    }
    const actionCodes = new Set();
    for (const action of resource.actions) {
      if (!action || !action.action_code || !action.label_ref) {
        findings.push(createFinding({
          rule: 'compiled-resource-action-invalid',
          subject: resource.resource_code,
          message: `compiled resource ${resource.resource_code} contains an invalid action`,
        }));
      } else if (actionCodes.has(action.action_code)) {
        findings.push(createFinding({
          rule: 'compiled-resource-action-duplicate',
          subject: `${resource.resource_code}.${action.action_code}`,
          message: `compiled resource action is registered more than once: ${resource.resource_code}.${action.action_code}`,
        }));
      } else {
        actionCodes.add(action.action_code);
      }
    }
  }

  for (const operation of evidence.operations) {
    if (operation.authorization?.kind !== 'resource_action') {
      continue;
    }
    const resource = resourceByCode.get(operation.authorization.resource_code);
    if (!resource) {
      findings.push(createFinding({
        rule: 'compiled-resource-reference-missing',
        subject: operation.operation_id,
        message: `operation ${operation.operation_id} references unregistered resource ${operation.authorization.resource_code}`,
      }));
      continue;
    }
    if (!(resource.actions || []).some((action) => action?.action_code === operation.authorization.action_code)) {
      findings.push(createFinding({
        rule: 'compiled-resource-action-reference-missing',
        subject: operation.operation_id,
        message: `operation ${operation.operation_id} references unregistered action ${operation.authorization.resource_code}.${operation.authorization.action_code}`,
      }));
    }
  }

  if (!Array.isArray(evidence.routeAssembly)) {
    findings.push(createFinding({
      rule: 'compiled-route-assembly-missing',
      message: 'compiled evidence must include route_assembly ownership bindings',
    }));
    return;
  }

  const assembledRoutes = new Map();
  for (const binding of evidence.routeAssembly) {
    const route = binding?.route;
    const ownership = binding?.ownership;
    if (!route || !ownership) {
      findings.push(createFinding({
        rule: 'compiled-route-assembly-binding-invalid',
        message: 'compiled route assembly contains a binding without route and explicit ownership',
      }));
      continue;
    }
    if (!route.path.startsWith('/api/console')) {
      findings.push(createFinding({
        rule: 'compiled-route-assembly-outside-console',
        subject: routeDisplay(route),
        message: `assembled route ${routeDisplay(route)} is outside /api/console`,
      }));
    }
    const key = routeKey(route);
    if (assembledRoutes.has(key)) {
      findings.push(createFinding({
        rule: 'compiled-route-ownership-duplicate',
        subject: key,
        message: `assembled route ${routeDisplay(route)} has duplicate ownership bindings`,
      }));
      continue;
    }
    assembledRoutes.set(key, { route, ownership });

    if (!interfaceByRoute.has(key)) {
      findings.push(createFinding({
        rule: 'compiled-interface-route-missing',
        subject: routeDisplay(route),
        message: `assembled route ${routeDisplay(route)} has no static interface metadata`,
      }));
    }

    const expected = expectedRoutes.get(key);
    if (!expected) {
      findings.push(createFinding({
        rule: 'compiled-inventory-route-unregistered',
        subject: routeDisplay(route),
        message: `assembled route ${routeDisplay(route)} is not owned by a compiled operation`,
      }));
      continue;
    }

    if (ownership.kind === 'authenticated') {
      if (expected.operation.authorization?.kind !== 'authenticated') {
        findings.push(createFinding({
          rule: 'compiled-route-ownership-mismatch',
          subject: routeDisplay(route),
          message: `assembled route ${routeDisplay(route)} is Authenticated but compiled owner is ${expected.operation.operation_id}`,
        }));
      }
    } else if (ownership.kind === 'console_operation') {
      const ownershipMatches = ownership.operation_id === expected.operation.operation_id
        || ownership.operation_id === expected.operation.authorization_profile_id;
      if (!ownershipMatches) {
        findings.push(createFinding({
          rule: 'compiled-route-ownership-mismatch',
          subject: routeDisplay(route),
          message: `assembled route ${routeDisplay(route)} declares ${ownership.operation_id}, expected ${expected.operation.operation_id}`,
        }));
      }
      const owner = ownershipMatches ? expected.operation : operationById.get(ownership.operation_id);
      if (!owner) {
        findings.push(createFinding({
          rule: 'compiled-route-owner-missing',
          subject: routeDisplay(route),
          message: `assembled route ${routeDisplay(route)} references unregistered operation ${ownership.operation_id}`,
        }));
      } else if (owner.lifecycle !== 'active') {
        findings.push(createFinding({
          rule: 'compiled-inactive-owner-route',
          subject: routeDisplay(route),
          message: `assembled route ${routeDisplay(route)} references inactive operation ${ownership.operation_id}`,
        }));
      }
    } else {
      findings.push(createFinding({
        rule: 'compiled-route-ownership-invalid',
        subject: routeDisplay(route),
        message: `assembled route ${routeDisplay(route)} does not use Authenticated or ConsoleOperation ownership`,
      }));
    }
  }

  for (const [key, expected] of expectedRoutes) {
    if (!assembledRoutes.has(key)) {
      const missing = {
        kind: 'route-ownership',
        key: `${expected.operation.operation_id}: ${routeDisplay(expected.route)}`,
        route: expected.route,
        operation_id: expected.operation.operation_id,
      };
      diff.missing.push(missing);
      findings.push(createFinding({
        rule: 'compiled-inventory-route-missing',
        subject: missing.key,
        message: `compiled operation route has no route_assembly ownership: ${missing.key}`,
      }));
    }
  }

  for (const [key, item] of interfaceByRoute) {
    if (!assembledRoutes.has(key)) {
      findings.push(createFinding({
        rule: 'compiled-interface-route-unmounted',
        subject: item.interface_id,
        message: `compiled interface ${item.interface_id} references an unmounted route ${routeDisplay(item.route)}`,
      }));
    }
  }
}

function flattenLocale(value, prefix = '', output = new Set()) {
  if (!value || typeof value !== 'object') {
    return output;
  }
  for (const [key, child] of Object.entries(value)) {
    const next = prefix ? `${prefix}.${key}` : key;
    output.add(next);
    if (child && typeof child === 'object') {
      flattenLocale(child, next, output);
    }
  }
  return output;
}

function collectI18nRefs(evidence) {
  const refs = [];
  for (const resource of evidence?.resources || []) {
    if (resource?.label_ref) refs.push({ ref: resource.label_ref, subject: resource.resource_code });
    if (resource?.description_ref) refs.push({ ref: resource.description_ref, subject: resource.resource_code });
    for (const action of resource?.actions || []) {
      if (action?.label_ref) refs.push({ ref: action.label_ref, subject: `${resource.resource_code}.${action.action_code}` });
      if (action?.description_ref) refs.push({ ref: action.description_ref, subject: `${resource.resource_code}.${action.action_code}` });
    }
  }
  return [...new Map(refs.map((item) => [`${item.subject}:${item.ref}`, item])).values()];
}

function validateLocaleEvidence(evidence, localeSources, findings) {
  if (!evidence) {
    return;
  }
  const refs = collectI18nRefs(evidence);
  for (const locale of DEFAULT_LOCALES) {
    const source = localeSources?.[locale];
    if (!source || !source.data) {
      findings.push(createFinding({
        rule: 'locale-evidence-missing',
        source: 'locale-files',
        file: source?.file || null,
        subject: locale,
        message: `locale evidence for ${locale} is missing or invalid`,
      }));
      continue;
    }
    const keys = flattenLocale(source.data);
    for (const item of refs) {
      if (!keys.has(item.ref)) {
        findings.push(createFinding({
          rule: 'locale-ref-missing',
          source: 'locale-files',
          file: source.file,
          subject: `${locale}:${item.ref}`,
          message: `${locale} does not provide compiled i18n ref ${item.ref}`,
        }));
      }
    }
  }
}

function evaluateMigrationEvidence(migration, operationIds, diff, findings) {
  if (!migration || typeof migration !== 'object') {
    return;
  }

  const unknownPermissions = [
    ...(migration.unknown_permissions || []),
    ...(migration.unregistered_permissions || []),
  ];
  for (const permission of unknownPermissions) {
    diff.migration.unknown_permissions.push(permission);
    findings.push(createFinding({
      rule: 'migration-unknown-legacy-permission',
      source: 'migration-fixture',
      subject: permission,
      message: `legacy permission has no deterministic compiled operation mapping: ${permission}`,
    }));
  }

  const legacyPermissions = migration.legacy_permissions || migration.legacyPermissions || [];
  for (const entry of legacyPermissions) {
    const operationId = typeof entry === 'string' ? null : entry?.operation_id || entry?.operationId;
    if (!operationId || !operationIds.has(operationId)) {
      const permission = typeof entry === 'string' ? entry : entry?.permission || '(unknown)';
      findings.push(createFinding({
        rule: 'migration-unknown-legacy-permission',
        source: 'migration-fixture',
        subject: permission,
        message: `legacy permission cannot be projected to an active compiled operation: ${permission}`,
      }));
    }
    if (entry?.scope === 'system_all' || entry?.scope === 'system') {
      findings.push(createFinding({
        rule: 'migration-system-all-forbidden',
        source: 'migration-fixture',
        subject: operationId || entry.permission,
        message: 'console migration cannot project legacy permission scope to system_all',
      }));
  }
  }

  const deltas = migration.authorization_delta
    || migration.authorization_deltas
    || migration.delta
    || [];
  for (const delta of deltas) {
    diff.migration.authorization_delta.push(delta);
    findings.push(createFinding({
      rule: 'migration-authorization-delta',
      source: 'migration-fixture',
      subject: delta.operation_id || delta.operationId || delta.role_id || delta.roleId || null,
      message: 'migration authorization matrix is not equivalent before and after cutover',
    }));
  }

  if (migration.rollback_verified === false || migration.rollbackVerified === false) {
    findings.push(createFinding({
      rule: 'migration-rollback-not-verified',
      source: 'migration-fixture',
      message: 'migration fixture does not provide verified rollback evidence',
    }));
  }
}

function addSourceWarning(findings, rule, file, message, subject = null) {
  findings.push(createFinding({
    rule,
    severity: 'warning',
    source: 'source-scan',
    file,
    subject,
    message,
  }));
}

function evaluateSourceSafety(sourceFiles, findings) {
  const rolePanel = sourceFiles?.rolePermissionPanel;
  if (rolePanel?.source) {
    const forbiddenPatterns = [
      { pattern: /\bRESOURCE_MAP\b/u, subject: 'RESOURCE_MAP' },
      { pattern: /\b(?:OPERATION_REGISTRY|CONSOLE_OPERATION_REGISTRY|operationRegistry)\b/u, subject: 'operation registry constant' },
      { pattern: /基础通用|系统管理|Agent\s*应用|basic\s+general|agent\s+application/iu, subject: 'legacy technical permission category' },
    ];
    for (const item of forbiddenPatterns) {
      if (item.pattern.test(rolePanel.source)) {
        addSourceWarning(
          findings,
          'frontend-role-permission-legacy-map',
          rolePanel.file,
          `RolePermissionPanel contains ${item.subject}; compiled backend policy inventory must remain its only permission definition`,
          item.subject
        );
      }
    }
  }

  const middleware = sourceFiles?.consoleMiddleware;
  if (middleware?.source) {
    if (/legacy[_\s-]*permission[_\s-]*(?:code|grant)\b|permission_code\s*\(/iu.test(middleware.source)) {
      addSourceWarning(
        findings,
        'runtime-legacy-permission-fallback',
        middleware.file,
        'console middleware source contains a legacy permission/fallback-shaped path; compiled route access must decide authorization',
        'legacy permission mapping'
      );
    }
    if (/console_route_unregistered[\s\S]{0,240}next\.run|next\.run[\s\S]{0,240}console_route_unregistered/iu.test(middleware.source)) {
      addSourceWarning(
        findings,
        'runtime-unregistered-route-fallback',
        middleware.file,
        'unregistered console route appears to continue through next.run; compiled failures must deny by default',
        'unregistered console route'
      );
    }
  }

  const routeHost = sourceFiles?.routeHost;
  if (routeHost?.source && /\.route\s*\(\s*["']\/api\/console/u.test(routeHost.source)) {
    addSourceWarning(
      findings,
      'console-route-outside-assembly',
      routeHost.file,
      'source contains a direct /api/console route registration outside the compiled route_assembly; verify it with the compiled gate',
      '/api/console route'
    );
  }

  const legacyMapping = sourceFiles?.legacyMapping;
  if (legacyMapping?.source && /operation_permission_code|permission_code\s*\([^)]*\.(?:all|own)/iu.test(legacyMapping.source)) {
    addSourceWarning(
      findings,
      'runtime-legacy-permission-fallback',
      legacyMapping.file,
      'source still exposes legacy permission mapping helpers; this regex result is advisory and does not establish runtime authorization truth',
      'legacy permission mapping helper'
    );
  }
}

function evaluateConsoleOperationRegistryHygiene({
  repoRoot = getRepoRoot(),
  inventory,
  compiledChecks = { status: 0, authoritative: true, commands: [] },
} = {}) {
  const findings = [];
  const diff = emptyDiff();
  const current = inventory?.current || inventory?.inventory || (inventory?.operations ? inventory : null);
  const baseline = inventory?.baseline || null;

  if (!compiledChecks?.authoritative) {
    findings.push(createFinding({
      rule: 'compiled-evidence-not-authoritative',
      message: 'compiled assembly evidence was not produced by the required cargo tests',
    }));
  }
  for (const command of compiledChecks?.commands || []) {
    if (command.status !== 'passed' || command.exitCode !== 0) {
      findings.push(createFinding({
        rule: 'compiled-test-failure',
        subject: command.label,
        message: `compiled assembly test ${command.label} failed (exit ${command.exitCode ?? 'unknown'})`,
      }));
    }
  }

  if (!current) {
    findings.push(createFinding({
      rule: 'compiled-inventory-snapshot-missing',
      source: 'compiled-assembly',
      message: 'no compiled inventory snapshot was supplied; inventory, locale, and diff checks fail closed',
    }));
  } else {
    validateCompiledEvidence(current, findings, diff);
    validateLocaleEvidence(current, inventory.localeSources, findings);
    if (baseline) {
      const inventoryDiff = compareCompiledInventories(current, baseline);
      diff.missing.push(...inventoryDiff.missing);
      diff.expansion.push(...inventoryDiff.expansion);
      diff.regrouping.push(...inventoryDiff.regrouping);
    } else {
      findings.push(createFinding({
        rule: 'compiled-inventory-baseline-missing',
        source: 'compiled-assembly',
        message: 'no baseline compiled inventory was supplied; permission expansion diff fails closed',
      }));
    }

    for (const item of diff.missing) {
      if (item.kind === 'route-ownership') {
        continue;
      }
      findings.push(createFinding({
        rule: 'compiled-inventory-diff-missing',
        subject: item.key,
        message: `compiled inventory gap detected: ${item.kind} ${item.key}`,
      }));
    }
    if (diff.expansion.length > 0) {
      findings.push(createFinding({
        rule: 'permission-expansion-detected',
        subject: diff.expansion.map((item) => item.key).join(', '),
        message: `compiled inventory contains ${diff.expansion.length} permission expansion diff item(s); CI must review an explicit delta`,
      }));
    }
  }

  const operationIds = new Set((current?.operations || []).map((operation) => operation.operation_id));
  evaluateMigrationEvidence(current?.migration, operationIds, diff, findings);
  evaluateSourceSafety(inventory?.sourceFiles, findings);

  const errors = findings.filter((finding) => finding.severity === 'error').length;
  const warnings = findings.filter((finding) => finding.severity === 'warning').length;
  const localeReferenceCount = collectI18nRefs(current).length;
  const localeFailed = findings.some((finding) => (
    finding.rule === 'locale-evidence-missing' || finding.rule === 'locale-ref-missing'
  ));
  return {
    generatedAt: new Date().toISOString(),
    summary: {
      findings: findings.length,
      errors,
      warnings,
      missing: diff.missing.length,
      expansions: diff.expansion.length,
      regrouping: diff.regrouping.length,
      compiledChecksPassed: (compiledChecks?.commands || []).filter((item) => item.status === 'passed').length,
      compiledChecksFailed: (compiledChecks?.commands || []).filter((item) => item.status !== 'passed').length,
    },
    compiled_checks: compiledChecks,
    inventory: current
      ? {
        schema_version: current.schemaVersion,
        operation_count: current.operations?.length || 0,
        resource_count: current.resources?.length || 0,
        route_assembly_count: current.routeAssembly?.length || 0,
        source: inventory.compiledInventoryPath,
      }
      : null,
    checks: {
      inventory: {
        status: current ? 'passed' : 'failed',
        source: inventory?.compiledInventoryPath || null,
        operation_count: current?.operations?.length || 0,
        resource_count: current?.resources?.length || 0,
        route_assembly_count: current?.routeAssembly?.length || 0,
      },
      locale: {
        status: current && !localeFailed ? 'passed' : 'failed',
        locales: DEFAULT_LOCALES.map((locale) => ({
          locale,
          source: inventory?.localeSources?.[locale]?.file || null,
        })),
        reference_count: localeReferenceCount,
      },
      diff: {
        status: current && baseline ? 'passed' : 'failed',
        baseline_source: inventory?.baselineInventoryPath || null,
        compared_operation_count: baseline?.operations?.length || 0,
        compared_resource_count: baseline?.resources?.length || 0,
      },
    },
    diff,
    findings,
    repo_root: repoRoot,
  };
}

function renderMarkdown(report) {
  const lines = [
    '# Console Operation Registry Hygiene',
    '',
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Warnings: ${report.summary.warnings}`,
    `- Missing diff items: ${report.summary.missing}`,
    `- Permission expansion diff items: ${report.summary.expansions}`,
    '',
    '## Compiled Assembly Evidence',
    '',
    `- Authoritative: ${report.compiled_checks.authoritative ? 'yes' : 'no'}`,
    `- Commands passed: ${report.summary.compiledChecksPassed}`,
    `- Commands failed: ${report.summary.compiledChecksFailed}`,
    '',
    '## Missing / Gap Diff',
    '',
  ];

  if (report.diff.missing.length === 0) {
    lines.push('No missing or gap diff items.');
  } else {
    lines.push('| Kind | Key |');
    lines.push('| --- | --- |');
    for (const item of report.diff.missing) {
      lines.push(`| ${item.kind} | ${String(item.key).replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Permission Expansion Diff', '');
  if (report.diff.expansion.length === 0) {
    lines.push('No permission expansion diff items.');
  } else {
    lines.push('| Kind | Key |');
    lines.push('| --- | --- |');
    for (const item of report.diff.expansion) {
      lines.push(`| ${item.kind} | ${String(item.key).replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Regrouping Diff', '');
  if (report.diff.regrouping.length === 0) {
    lines.push('No policy-group regrouping.');
  } else {
    lines.push('| Kind | Key | Before | After |');
    lines.push('| --- | --- | --- | --- |');
    for (const item of report.diff.regrouping) {
      lines.push(`| ${item.kind} | ${item.key} | ${JSON.stringify(item.before)} | ${JSON.stringify(item.after)} |`);
    }
  }

  lines.push('', '## Findings', '');
  if (report.findings.length === 0) {
    lines.push('No findings.');
  } else {
    lines.push('| Severity | Rule | Source | Subject | File | Message |');
    lines.push('| --- | --- | --- | --- | --- | --- |');
    for (const finding of report.findings) {
      const message = String(finding.message).replace(/\|/gu, '\\|').replace(/\n/gu, '<br>');
      lines.push(
        `| ${finding.severity} | ${finding.rule} | ${finding.source} | ${finding.subject || ''} | ${finding.file || ''} | ${message} |`
      );
    }
  }

  return `${lines.join('\n')}\n`;
}

function writeReport({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS } = {}) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };
  const jsonReportPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownReportPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(jsonReportPath, `${JSON.stringify(reportForDisk, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownReportPath, renderMarkdown(reportForDisk), 'utf8');
  return { jsonReportPath, markdownReportPath, report: reportForDisk };
}

function parseCliArgs(argv = []) {
  const options = {
    help: false,
    compiledInventoryPath: null,
    baselineInventoryPath: null,
    localeDir: DEFAULT_LOCALE_DIR,
    maxFindings: DEFAULT_MAX_FINDINGS,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }
    if (arg === '--compiled-inventory' || arg === '--assembly') {
      options.compiledInventoryPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--baseline-inventory' || arg === '--baseline') {
      options.baselineInventoryPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--locale-dir') {
      options.localeDir = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--max-findings') {
      options.maxFindings = Number.parseInt(argv[index + 1], 10);
      index += 1;
      continue;
    }
    throw new Error(`Unknown console-operation-registry-hygiene option: ${arg}`);
  }

  if (!options.help) {
    for (const [name, value] of [
      ['--compiled-inventory', options.compiledInventoryPath],
      ['--baseline-inventory', options.baselineInventoryPath],
      ['--locale-dir', options.localeDir],
    ]) {
      if (value === undefined || value === '') {
        throw new Error(`${name} requires a value`);
      }
    }
    if (!Number.isInteger(options.maxFindings) || options.maxFindings < 1) {
      throw new Error('--max-findings must be a positive integer');
    }
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/console-operation-registry-hygiene/cli.js '
      + '[--compiled-inventory <path>] [--baseline-inventory <path>] '
      + '[--locale-dir <path>] [--max-findings <n>]\n'
      + 'Runs authoritative compiled console route tests, validates optional compiled inventory '
      + 'and reports route gaps, permission expansion diff, migration delta, i18n evidence, '
      + 'and advisory source hygiene checks.\n'
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
  const env = deps.env || process.env;
  let compiledChecks;
  let inventory;
  let report;

  try {
    const runChecks = deps.runCompiledChecksImpl || runCompiledAssemblyChecks;
    compiledChecks = await runChecks({
      repoRoot,
      env,
      writeStdout,
      writeStderr,
      spawnSyncImpl: deps.spawnSyncImpl,
    });
    const compiledInventoryPath = options.compiledInventoryPath
      || (deps.generateCompiledInventoryImpl || generateCompiledInventorySnapshot)({
        repoRoot,
        env,
        spawnSyncImpl: deps.spawnSyncImpl,
      });
    const baselineInventoryPath = options.baselineInventoryPath
      || DEFAULT_BASELINE_INVENTORY_PATH;
    inventory = (deps.collectInventoryImpl || collectConsoleOperationRegistryInventory)({
      repoRoot,
      compiledInventoryPath,
      baselineInventoryPath,
      localeDir: options.localeDir,
    });
    report = (deps.evaluateImpl || evaluateConsoleOperationRegistryHygiene)({
      repoRoot,
      inventory,
      compiledChecks,
    });
  } catch (error) {
    compiledChecks = compiledChecks || {
      status: 1,
      authoritative: false,
      commands: [],
    };
    report = {
      generatedAt: new Date().toISOString(),
      summary: {
        findings: 1,
        errors: 1,
        warnings: 0,
        missing: 0,
        expansions: 0,
        regrouping: 0,
        compiledChecksPassed: 0,
        compiledChecksFailed: 0,
      },
      compiled_checks: compiledChecks,
      inventory: null,
      diff: emptyDiff(),
      findings: [createFinding({
        rule: 'cli-input-failure',
        source: 'cli',
        message: error.message,
      })],
      repo_root: repoRoot,
    };
  }

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
    `[1flowbase-console-operation-registry-hygiene] ${report.summary.findings} findings `
      + `(${report.summary.errors} errors, ${report.summary.warnings} warnings). `
      + `Reports: ${relativeRepoPath(repoRoot, jsonReportPath)}, `
      + `${relativeRepoPath(repoRoot, markdownReportPath)}\n`
  );
  for (const finding of reportForDisk.findings.filter((item) => item.severity === 'error')) {
    writeStderr(
      `[console-operation-registry-hygiene:${finding.rule}] ${finding.subject || ''} `
        + `${finding.message}\n`
    );
  }

  return report.summary.errors > 0 ? 1 : 0;
}

module.exports = {
  DEFAULT_BASELINE_INVENTORY_PATH,
  INVENTORY_SCHEMA_VERSION,
  buildCompiledAssemblyCommands,
  collectConsoleOperationRegistryInventory,
  compareCompiledInventories,
  evaluateConsoleOperationRegistryHygiene,
  generateCompiledInventorySnapshot,
  main,
  normalizeCompiledEvidence,
  parseCargoTestCounts,
  parseCliArgs,
  renderMarkdown,
  routeKey,
  runCompiledAssemblyChecks,
  writeReport,
};
