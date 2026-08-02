const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');
const { pathToFileURL } = require('node:url');
const {
  loadRootCredentials,
  openTemporaryOwnerSession,
} = require('../page-debug/auth.js');

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?$/;
const ALLOWED_ROOT_ENTRIES = new Set(['manifest.json', 'tools', 'instances', 'connections']);

function nextPatch(version) {
  const match = SEMVER.exec(version);
  if (!match) throw new Error(`target manifest bundle_version must be semantic version: ${version}`);
  return `${match[1]}.${match[2]}.${Number(match[3]) + 1}`;
}

function readTargetIdentity(target) {
  const manifestPath = path.join(target, 'manifest.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const organizationDirectory = path.basename(path.dirname(target));
  if (!organizationDirectory.startsWith('@') || organizationDirectory.slice(1) !== manifest.organization || path.basename(target) !== manifest.bundle_id) {
    throw new Error('target path identity does not match manifest organization/bundle_id');
  }
  if (typeof manifest.locale !== 'string' || !manifest.locale) throw new Error('target manifest locale is required');
  return {
    organization: manifest.organization,
    bundle_id: manifest.bundle_id,
    bundle_version: nextPatch(manifest.bundle_version),
    locale: manifest.locale,
  };
}

function zipEntries(bytes) {
  const centralModes = centralDirectoryModes(bytes);
  const entries = [];
  let offset = 0;
  while (offset + 4 <= bytes.length && bytes.readUInt32LE(offset) === 0x04034b50) {
    if (offset + 30 > bytes.length) throw new Error('truncated ZIP local header');
    const flags = bytes.readUInt16LE(offset + 6);
    const method = bytes.readUInt16LE(offset + 8);
    const compressedSize = bytes.readUInt32LE(offset + 18);
    const uncompressedSize = bytes.readUInt32LE(offset + 22);
    const nameLength = bytes.readUInt16LE(offset + 26);
    const extraLength = bytes.readUInt16LE(offset + 28);
    if ((flags & 0x08) !== 0) throw new Error('ZIP data descriptors are not supported');
    if ((flags & 0x01) !== 0) throw new Error('encrypted ZIP entries are not supported');
    const nameStart = offset + 30;
    const dataStart = nameStart + nameLength + extraLength;
    const dataEnd = dataStart + compressedSize;
    if (dataEnd > bytes.length) throw new Error('truncated ZIP entry');
    const name = bytes.subarray(nameStart, nameStart + nameLength).toString('utf8');
    const mode = centralModes.get(name);
    if (mode !== undefined && (mode & 0o170000) === 0o120000) {
      throw new Error(`ZIP symlink entries are not allowed: ${name}`);
    }
    const compressed = bytes.subarray(dataStart, dataEnd);
    let contents;
    if (method === 0) contents = Buffer.from(compressed);
    else if (method === 8) contents = zlib.inflateRawSync(compressed);
    else throw new Error(`unsupported ZIP compression method ${method}`);
    if (contents.length !== uncompressedSize) throw new Error(`ZIP size mismatch for ${name}`);
    entries.push({ name, contents });
    offset = dataEnd;
  }
  if (entries.length === 0) throw new Error('archive contains no ZIP entries');
  return entries;
}

function centralDirectoryModes(bytes) {
  const minimum = Math.max(0, bytes.length - 65_557);
  let eocd = -1;
  for (let offset = bytes.length - 22; offset >= minimum; offset -= 1) {
    if (bytes.readUInt32LE(offset) === 0x06054b50) { eocd = offset; break; }
  }
  if (eocd === -1) throw new Error('ZIP end-of-central-directory record is missing');
  const total = bytes.readUInt16LE(eocd + 10);
  let offset = bytes.readUInt32LE(eocd + 16);
  const modes = new Map();
  for (let index = 0; index < total; index += 1) {
    if (offset + 46 > bytes.length || bytes.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error('invalid ZIP central directory');
    }
    const nameLength = bytes.readUInt16LE(offset + 28);
    const extraLength = bytes.readUInt16LE(offset + 30);
    const commentLength = bytes.readUInt16LE(offset + 32);
    const externalAttributes = bytes.readUInt32LE(offset + 38);
    const name = bytes.subarray(offset + 46, offset + 46 + nameLength).toString('utf8');
    if (modes.has(name)) throw new Error(`duplicate ZIP entry: ${name}`);
    modes.set(name, externalAttributes >>> 16);
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return modes;
}

function safeArchivePath(root, name) {
  if (!name || name.includes('\\') || name.includes('\0') || path.posix.isAbsolute(name)) {
    throw new Error(`unsafe ZIP entry path: ${name}`);
  }
  const normalized = path.posix.normalize(name.replace(/\/+$/, ''));
  if (normalized === '..' || normalized.startsWith('../')) throw new Error(`unsafe ZIP entry path: ${name}`);
  const [top] = normalized.split('/');
  if (!ALLOWED_ROOT_ENTRIES.has(top)) throw new Error(`unexpected ZIP entry: ${name}`);
  const destination = path.resolve(root, ...normalized.split('/'));
  if (destination !== root && !destination.startsWith(`${root}${path.sep}`)) throw new Error(`unsafe ZIP entry path: ${name}`);
  return destination;
}

function extractBundleArchive(bytes, destination) {
  for (const entry of zipEntries(bytes)) {
    const output = safeArchivePath(destination, entry.name);
    if (entry.name.endsWith('/')) {
      fs.mkdirSync(output, { recursive: true });
      continue;
    }
    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.writeFileSync(output, entry.contents, { flag: 'wx' });
  }
}

async function officialValidator(target, dependencies) {
  if (dependencies.validateSource) return dependencies.validateSource(target);
  let current = target;
  while (path.dirname(current) !== current) {
    const candidate = path.join(current, 'scripts', 'update-mcp-catalog.mjs');
    if (fs.existsSync(candidate)) {
      const module = await import(`${pathToFileURL(candidate).href}?validator=${Date.now()}`);
      return module.buildMcpBundleSource(target);
    }
    current = path.dirname(current);
  }
  throw new Error('official MCP source validator not found above target');
}

function validateExportedManifest(target, identity) {
  const manifest = JSON.parse(fs.readFileSync(path.join(target, 'manifest.json'), 'utf8'));
  for (const field of ['organization', 'bundle_id', 'bundle_version', 'locale']) {
    if (manifest[field] !== identity[field]) throw new Error(`exported manifest ${field} does not match request`);
  }
  if (!SEMVER.test(manifest.exported_from_system_version || '')) {
    throw new Error('exported manifest exported_from_system_version must be semantic version');
  }
  if (manifest.minimum_host_version !== manifest.exported_from_system_version) {
    throw new Error('exported manifest minimum_host_version must equal exported_from_system_version');
  }
  return manifest;
}

function replaceDirectoryAtomically(staged, target, dependencies = {}) {
  const rename = dependencies.renameSync || fs.renameSync;
  const backup = `${target}.backup-${process.pid}-${Date.now()}`;
  let movedOld = false;
  try {
    rename(target, backup);
    movedOld = true;
    rename(staged, target);
    fs.rmSync(backup, { recursive: true, force: true });
  } catch (error) {
    if (movedOld && !fs.existsSync(target) && fs.existsSync(backup)) rename(backup, target);
    throw error;
  }
}

async function exportMcpInstanceToOfficial(options, dependencies = {}) {
  const target = path.resolve(options.target);
  const identity = readTargetIdentity(target);
  const repoRoot = path.resolve(__dirname, '../../..');
  const credentials = (dependencies.loadRootCredentials || loadRootCredentials)({ repoRoot });
  const openSession = dependencies.openTemporaryOwnerSession || openTemporaryOwnerSession;
  const fetchImpl = dependencies.fetchImpl || globalThis.fetch;
  let session;
  const temporaryRoot = fs.mkdtempSync(path.join(path.dirname(target), '.mcp-export-'));
  const staged = path.join(temporaryRoot, 'bundle');
  fs.mkdirSync(staged);
  try {
    session = await openSession({
      apiBaseUrl: options.apiBaseUrl.replace(/\/+$/, ''),
      account: credentials.account,
      password: credentials.password,
      fetchImpl,
    });
    const response = await fetchImpl(`${options.apiBaseUrl.replace(/\/+$/, '')}/api/console/mcp/instances/${encodeURIComponent(options.instanceId)}/bundles/export`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        cookie: session.cookie,
        'x-csrf-token': session.csrfToken,
      },
      body: JSON.stringify(identity),
    });
    if (!response.ok) throw new Error(`MCP instance export failed: ${response.status} ${(await response.text()).slice(0, 500)}`.trim());
    const bytes = Buffer.from(await response.arrayBuffer());
    extractBundleArchive(bytes, staged);
    const manifest = validateExportedManifest(staged, identity);
    await officialValidator(staged, dependencies);
    await session.dispose();
    session = null;
    replaceDirectoryAtomically(staged, target, dependencies);
    return {
      instance_id: options.instanceId,
      target,
      bundle_version: identity.bundle_version,
      exported_from_system_version: manifest.exported_from_system_version,
      committed: false,
      pushed: false,
    };
  } finally {
    try {
      if (session) await session.dispose();
    } finally {
      fs.rmSync(temporaryRoot, { recursive: true, force: true });
    }
  }
}

module.exports = {
  exportMcpInstanceToOfficial,
  extractBundleArchive,
  nextPatch,
  readTargetIdentity,
  replaceDirectoryAtomically,
  validateExportedManifest,
  zipEntries,
};
