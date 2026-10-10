const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { isDeepStrictEqual } = require('node:util');
const { loadRootCredentials, openTemporaryOwnerSession } = require('../page-debug/auth.js');

const CONFIG_FILE = 'export.config.json';
const SELECTION_KEYS = ['page_ids', 'application_ids', 'data_model_ids', 'mcp_instance_ids', 'i18n_keys'];
function selectionOf(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('selection must be an object');
  if (Object.keys(value).some((key) => !SELECTION_KEYS.includes(key))) throw new Error('unknown selection field');
  const result = {};
  for (const key of SELECTION_KEYS) {
    const values = value[key] ?? [];
    if (!Array.isArray(values) || values.some((id) => typeof id !== 'string' || !id)) throw new Error(`invalid selection ${key}`);
    result[key] = [...new Set(values)];
  }
  if (!Object.values(result).some((ids) => ids.length)) throw new Error('select at least one resource');
  return result;
}

async function archiveTools(target) {
  let directory = path.dirname(target);
  while (path.dirname(directory) !== directory) {
    const modulePath = path.join(directory, 'scripts/application-template/archive.mjs');
    if (fs.existsSync(modulePath)) return import(pathToFileURL(modulePath).href);
    directory = path.dirname(directory);
  }
  throw new Error('target must be inside the official plugin repository containing scripts/application-template/archive.mjs');
}

function readConfig(options) {
  const target = path.resolve(options.target);
  const configPath = path.join(target, CONFIG_FILE);
  const existing = fs.existsSync(configPath) ? JSON.parse(fs.readFileSync(configPath, 'utf8')) : {};
  const templateId = `${path.basename(path.dirname(target))}/${path.basename(target)}`;
  if (!/^@[a-zA-Z0-9._-]+\/[a-zA-Z0-9._-]+$/.test(templateId)) throw new Error('target must end with @organization/template-name');
  if (existing.template_id && existing.template_id !== templateId) throw new Error('saved template identity does not match target');
  const apiBaseUrl = options.apiBaseUrl || existing.api_base_url || 'http://127.0.0.1:7800';
  const url = new URL(apiBaseUrl);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash || url.pathname !== '/') throw new Error('api-base-url must be an HTTP origin without credentials');
  return { target, schema_version: '1flowbase.application-template-export/v1', template_id: templateId,
    api_base_url: url.origin, name: options.name || existing.name || path.basename(target),
    description: options.description ?? existing.description ?? '',
    selection: selectionOf(options.selection || existing.selection),
  };
}

function replaceDirectory(staged, target, rename = fs.renameSync) {
  const backup = `${target}.backup-${process.pid}-${Date.now()}`;
  const hadTarget = fs.existsSync(target);
  if (hadTarget) rename(target, backup);
  try { rename(staged, target); }
  catch (error) {
    if (hadTarget) rename(backup, target);
    throw error;
  }
  if (hadTarget) fs.rmSync(backup, { recursive: true, force: true });
}

function contentWithoutExportTime(pkg) {
  const value = structuredClone(pkg);
  delete value.release;
  // MCP export time is provenance, not a change to the selected definitions.
  if (value.mcp_bundle?.manifest) delete value.mcp_bundle.manifest.exported_at;
  return value;
}

async function exportApplicationTemplate(options, dependencies = {}) {
  const config = readConfig(options);
  const tools = dependencies.archiveTools || await archiveTools(config.target);
  const previous = fs.existsSync(path.join(config.target, 'manifest.json')) ? await tools.readPackage(config.target) : null;
  const repoRoot = options.repoRoot || path.resolve(__dirname, '../../..');
  const credentials = (dependencies.loadRootCredentials || loadRootCredentials)({ repoRoot });
  const fetchImpl = dependencies.fetchImpl || globalThis.fetch;
  const openSession = dependencies.openTemporaryOwnerSession || openTemporaryOwnerSession;
  fs.mkdirSync(path.dirname(config.target), { recursive: true });
  const temporary = fs.mkdtempSync(path.join(path.dirname(config.target), '.application-template-export-'));
  const staged = path.join(temporary, path.basename(config.target));
  let session;
  try {
    session = await openSession({ apiBaseUrl: config.api_base_url, ...credentials, fetchImpl });
    const response = await fetchImpl(`${config.api_base_url}/api/console/settings/system-templates/export`, {
      method: 'POST', headers: { 'content-type': 'application/json', cookie: session.cookie, 'x-csrf-token': session.csrfToken },
      body: JSON.stringify(config.selection),
    });
    if (!response.ok) throw new Error(`template export failed: HTTP ${response.status} ${(await response.text()).slice(0, 500)}`);
    const body = await response.json();
    const pkg = body.data;
    if (!['1flowbase.portable-template/v1', '1flowbase.portable-template/v2'].includes(pkg?.schema_version)) throw new Error('unexpected template export response');
    const oldContent = previous && contentWithoutExportTime(previous);
    const content = contentWithoutExportTime(pkg);
    const changed = !previous || !isDeepStrictEqual(oldContent, content) || previous.release.name !== config.name || previous.release.description !== config.description;
    let release = previous?.release;
    if (changed) {
      const health = await fetchImpl(`${config.api_base_url}/health`);
      if (!health.ok) throw new Error('cannot determine source system version');
      const { version } = await health.json();
      if (typeof version !== 'string' || !version) throw new Error('source health response has no version');
      release = { template_id: config.template_id, release_version: (previous?.release?.release_version || 0) + 1,
        name: config.name, description: config.description, exported_from_system_version: version, exported_at: new Date().toISOString() };
    }
    await tools.splitPackage(changed ? { ...pkg, release } : previous, staged);
    await tools.readPackage(staged);
    const { target, ...saved } = config;
    fs.writeFileSync(path.join(staged, CONFIG_FILE), `${JSON.stringify(saved, null, 2)}\n`);
    // Preserve human documentation; generated catalog-entry is refreshed by the publisher.
    const readme = path.join(target, 'README.md');
    if (fs.existsSync(readme)) fs.copyFileSync(readme, path.join(staged, 'README.md'));
    if (!changed && fs.existsSync(path.join(target, 'catalog-entry.json'))) fs.copyFileSync(path.join(target, 'catalog-entry.json'), path.join(staged, 'catalog-entry.json'));
    await session.dispose();
    session = null;
    replaceDirectory(staged, target, dependencies.renameSync);
    return { template_id: config.template_id, release_version: release.release_version, changed, target,
      configuration: path.join(target, CONFIG_FILE), committed: false, pushed: false };
  } finally {
    try { if (session) await session.dispose(); }
    finally { fs.rmSync(temporary, { recursive: true, force: true }); }
  }
}

module.exports = { exportApplicationTemplate, readConfig, replaceDirectory, selectionOf };
