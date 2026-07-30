'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const SCHEMA = '1flowbase.local-ai-gateway-acceptance/v1';
const DEFAULT_MANIFEST = path.join(__dirname, 'manifest.json');
const FORBIDDEN_ACTION_WORDS = Object.freeze(['fetch', 'pull', 'clone', 'build', 'install', 'curl', 'wget']);
const LOCAL_ACTIONS = Object.freeze([
  Object.freeze({ owner: 'git', action: 'inspect-fixed-revisions-and-local-source-objects' }),
  Object.freeze({ owner: 'docker', action: 'create-and-probe-ephemeral-postgresql' }),
  Object.freeze({ owner: 'node', action: 'start-controlled-gateway-runtime-once' }),
  Object.freeze({ owner: 'tmux', action: 'run-eight-client-attempts' }),
  Object.freeze({ owner: 'node', action: 'clean-all-owned-resources' }),
]);

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function requiredObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} is required`);
  return value;
}

function validateArtifactContract(name, artifact) {
  requiredObject(artifact, `artifact ${name}`);
  if (typeof artifact.path === 'string') {
    if (!path.isAbsolute(artifact.path)) throw new Error(`artifact ${name} path must be absolute`);
    if (!/^[a-f0-9]{64}$/u.test(artifact.sha256)) throw new Error(`artifact ${name} SHA-256 is invalid`);
    return;
  }
  if (typeof artifact.directory !== 'string' || !path.isAbsolute(artifact.directory)) {
    throw new Error(`artifact ${name} directory must be absolute`);
  }
  if (typeof artifact.filename_pattern !== 'string' || !artifact.filename_pattern.trim()) {
    throw new Error(`artifact ${name} filename pattern is required`);
  }
  const pattern = new RegExp(artifact.filename_pattern, 'u');
  if (pattern.exec('probe')?.length === 1) {
    throw new Error(`artifact ${name} filename pattern must capture its SHA-256`);
  }
}

function resolveArtifactInventory(manifest) {
  const artifacts = Object.fromEntries(Object.entries(manifest.artifacts).map(([name, artifact]) => {
    if (typeof artifact.path === 'string') return [name, { ...artifact }];
    let pattern;
    try {
      pattern = new RegExp(artifact.filename_pattern, 'u');
    } catch (error) {
      throw new Error(`artifact ${name} filename pattern is invalid: ${error.message}`);
    }
    let entries;
    try {
      entries = fs.readdirSync(artifact.directory, { withFileTypes: true });
    } catch (error) {
      throw new Error(`artifact ${name} discovery directory is unavailable: ${error.message}`);
    }
    const matches = entries.flatMap((entry) => {
      if (!entry.isFile()) return [];
      const match = pattern.exec(entry.name);
      return match ? [{ path: path.join(artifact.directory, entry.name), sha256: match[1] }] : [];
    });
    if (matches.length !== 1) {
      throw new Error(`artifact ${name} discovery requires exactly one verified package, found ${matches.length}`);
    }
    if (!/^[a-f0-9]{64}$/u.test(matches[0].sha256 || '')) {
      throw new Error(`artifact ${name} filename did not capture a SHA-256`);
    }
    return [name, matches[0]];
  }));
  return { ...manifest, artifacts };
}

function loadManifest(filePath = DEFAULT_MANIFEST) {
  const resolved = path.resolve(filePath);
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(resolved, 'utf8'));
  } catch (error) {
    throw new Error(`local acceptance manifest is unavailable: ${error.message}`);
  }
  if (manifest.schema_version !== SCHEMA) throw new Error('local acceptance manifest schema mismatch');
  requiredObject(manifest.database, 'database contract');
  requiredObject(manifest.artifacts, 'artifact inventory');
  if (
    manifest.database.container !== 'docker-db-1'
    || manifest.database.image !== 'postgres:16-alpine'
    || manifest.database.host !== '127.0.0.1'
    || manifest.database.port !== 35432
  ) {
    throw new Error('local acceptance database contract must use docker-db-1 at 127.0.0.1:35432');
  }
  for (const [name, artifact] of Object.entries(manifest.artifacts)) {
    validateArtifactContract(name, artifact);
  }
  return manifest;
}

function verifyChecksums(manifest) {
  return Object.entries(manifest.artifacts).map(([name, artifact]) => {
    if (!fs.existsSync(artifact.path) || !fs.statSync(artifact.path).isFile()) {
      throw new Error(`artifact ${name} is missing`);
    }
    const actual = sha256File(artifact.path);
    if (actual !== artifact.sha256) throw new Error(`artifact ${name} checksum mismatch`);
    return { name, path: artifact.path, sha256: actual, bytes: fs.statSync(artifact.path).size };
  });
}

module.exports = {
  DEFAULT_MANIFEST,
  FORBIDDEN_ACTION_WORDS,
  LOCAL_ACTIONS,
  SCHEMA,
  loadManifest,
  resolveArtifactInventory,
  sha256File,
  verifyChecksums,
};
