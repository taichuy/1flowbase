const fs = require('node:fs');
const path = require('node:path');

const REGISTRATIONS = 'api/crates/access-control/src/settings_features/core_registrations.rs';
const CONTRACT = 'api/crates/access-control/src/settings_features.rs';

function validateDescriptor(value) {
  const text = (input, field) => {
    if (typeof input !== 'string' || !input.trim() || /[\r\n\x00]/u.test(input)) {
      throw new Error(`Invalid ${field}`);
    }
  };
  text(value.feature_id, 'feature_id');
  if (!/^system\.[a-z][a-z0-9.-]*$/u.test(value.feature_id)) {
    throw new Error('Core feature_id must use the system namespace');
  }
  const surface = value.console_surface;
  if (!surface) throw new Error('Missing console_surface');
  for (const key of ['route_id', 'surface_key', 'path', 'label_key', 'description_key']) {
    text(surface[key], `console_surface.${key}`);
  }
  if (!surface.path.startsWith('/settings/') || !Number.isSafeInteger(surface.order)) {
    throw new Error('Invalid settings surface path/order');
  }
  if (!Array.isArray(value.api_routes) || value.api_routes.length === 0) {
    throw new Error('api_routes must contain the actual API ownership');
  }
  const routes = new Set();
  for (const route of value.api_routes) {
    if (!['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].includes(route.method)) {
      throw new Error('Invalid API method');
    }
    text(route.path, 'api route path');
    if (!route.path.startsWith('/api/console/') || route.path.includes(':')) {
      throw new Error('API paths must use /api/console and {parameter} templates');
    }
    const key = `${route.method} ${route.path}`;
    if (routes.has(key)) throw new Error(`Duplicate route ${key}`);
    routes.add(key);
  }
  return value;
}

// Read only the existing canonical registration blocks, respecting quoted Rust strings.
// This is a source editor, not a second runtime registry or permission inventory.
function registrationBlocks(source, contractSource) {
  const constants = new Map(Array.from(
    contractSource.matchAll(/pub const ([A-Z][A-Z0-9_]*):\s*&str\s*=\s*"([^"]+)"/gu),
    (match) => [match[1], match[2]],
  ));
  const blocks = [];
  const marker = /SettingsFeatureRegistration\s*\{/gu;
  for (let match = marker.exec(source); match; match = marker.exec(source)) {
    let depth = 1;
    let quoted = false;
    let escaped = false;
    let end = marker.lastIndex;
    for (; end < source.length && depth > 0; end += 1) {
      const ch = source[end];
      if (quoted) {
        if (escaped) escaped = false;
        else if (ch === '\\') escaped = true;
        else if (ch === '"') quoted = false;
      } else if (ch === '"') quoted = true;
      else if (ch === '{') depth += 1;
      else if (ch === '}') depth -= 1;
    }
    if (depth !== 0) throw new Error('Unbalanced canonical registration');
    const body = source.slice(match.index, end);
    const identity = body.match(/feature_id\s*:\s*(?:"([^"]+)"|([A-Z][A-Z0-9_]*))/u);
    const featureId = identity && (identity[1] || constants.get(identity[2]));
    if (!featureId) throw new Error('Cannot resolve canonical feature identity');
    const routes = Array.from(
      body.matchAll(/\(\s*"(GET|POST|PUT|PATCH|DELETE)"\s*,\s*"([^"]+)"\s*,?\s*\)/gu),
      (route) => `${route[1]} ${route[2]}`,
    );
    blocks.push({ start: match.index, end, featureId, routes });
    marker.lastIndex = end;
  }
  return blocks;
}

function renderRegistration(descriptor) {
  const string = (value) => `${JSON.stringify(value)}.to_string()`;
  const surface = descriptor.console_surface;
  return `SettingsFeatureRegistration {
            feature_id: ${string(descriptor.feature_id)},
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
${['route_id', 'surface_key', 'path', 'label_key', 'description_key'].map((key) => `                ${key}: ${string(surface[key])},`).join('\n')}
                order: ${surface.order},
            },
            api_routes: settings_api_routes(&[
${descriptor.api_routes.map((route) => `                (${JSON.stringify(route.method)}, ${JSON.stringify(route.path)}),`).join('\n')}
            ]),
        }`;
}

function register(source, contractSource, input) {
  const descriptor = validateDescriptor(input);
  const blocks = registrationBlocks(source, contractSource);
  const matching = blocks.filter((block) => block.featureId === descriptor.feature_id);
  if (matching.length > 1) throw new Error('Duplicate feature registration in canonical source');
  const existing = new Map();
  for (const block of blocks) {
    if (block.featureId === descriptor.feature_id) continue;
    for (const route of block.routes) existing.set(route, block.featureId);
  }
  for (const route of descriptor.api_routes) {
    const key = `${route.method} ${route.path}`;
    if (existing.has(key)) throw new Error(`${key} already belongs to ${existing.get(key)}`);
  }
  const rendered = renderRegistration(descriptor);
  if (matching.length) {
    const block = matching[0];
    return source.slice(0, block.start) + rendered + source.slice(block.end);
  }
  const insertion = source.indexOf('vec![');
  if (insertion < 0) throw new Error('Canonical Core registration vector not found');
  const end = insertion + 'vec!['.length;
  return `${source.slice(0, end)}\n        ${rendered},${source.slice(end)}`;
}

function main(argv = [], deps = {}) {
  const write = deps.writeStdout || ((text) => process.stdout.write(text));
  if (argv.length === 0 || argv.includes('--help')) {
    write('Usage: node scripts/node/tooling.js settings-registry register <descriptor.json> [--dry-run]\n');
    write('Edits the canonical Core registration; verify the compiled inventory in the acceptance batch.\n');
    return 0;
  }
  if (argv[0] !== 'register' || !argv[1] || argv.slice(2).some((arg) => arg !== '--dry-run')) {
    throw new Error('Expected register <descriptor.json> [--dry-run]');
  }
  const repoRoot = deps.repoRoot || path.resolve(__dirname, '../../..');
  const sourcePath = path.join(repoRoot, REGISTRATIONS);
  const source = fs.readFileSync(sourcePath, 'utf8');
  const contract = fs.readFileSync(path.join(repoRoot, CONTRACT), 'utf8');
  const descriptor = JSON.parse(fs.readFileSync(path.resolve(repoRoot, argv[1]), 'utf8'));
  const next = register(source, contract, descriptor);
  if (!argv.includes('--dry-run')) fs.writeFileSync(sourcePath, next);
  write(`${JSON.stringify({ feature_id: descriptor.feature_id, path: REGISTRATIONS, changed: source !== next, dry_run: argv.includes('--dry-run') })}\n`);
  return 0;
}

module.exports = { main, register, registrationBlocks, validateDescriptor };
