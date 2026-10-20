const INVENTORY_SCHEMA_VERSION = '1flowbase.console-operation-inventory/v1';

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

module.exports = { INVENTORY_SCHEMA_VERSION, routeKey, routeDisplay, normalizeCompiledEvidence };
