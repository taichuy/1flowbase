'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const SCHEMA = '1flowbase.local-ai-gateway-acceptance/v1';
const DEFAULT_MANIFEST = path.join(__dirname, 'manifest.json');
const FORBIDDEN_ACTION_WORDS = Object.freeze(['fetch', 'pull', 'clone', 'build', 'install', 'curl', 'wget']);
const LOCAL_ACTIONS = Object.freeze([
  Object.freeze({ owner: 'git', action: 'inspect-fixed-revisions' }),
  Object.freeze({ owner: 'docker', action: 'create-and-probe-ephemeral-postgresql' }),
  Object.freeze({ owner: 'node', action: 'start-controlled-gateway-runtime-once' }),
  Object.freeze({ owner: 'node', action: 'run-three-pinned-acp-clients' }),
  Object.freeze({ owner: 'node', action: 'clean-all-owned-resources' }),
]);

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function requiredObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} is required`);
  return value;
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
  for (const required of ['apiServer', 'pluginRunner', 'openaiPackage', 'anthropicPackage']) {
    if (!manifest.artifacts[required]) throw new Error(`artifact ${required} is required`);
  }
  if (
    manifest.database.container !== 'docker-db-1'
    || manifest.database.image !== 'postgres:16-alpine'
    || manifest.database.host !== '127.0.0.1'
    || manifest.database.port !== 35432
  ) {
    throw new Error('local acceptance database contract must use docker-db-1 at 127.0.0.1:35432');
  }
  for (const [name, artifact] of Object.entries(manifest.artifacts)) {
    requiredObject(artifact, `artifact ${name}`);
    if (typeof artifact.path !== 'string' || !path.isAbsolute(artifact.path)) {
      throw new Error(`artifact ${name} path must be absolute`);
    }
    if (!/^[a-f0-9]{64}$/u.test(artifact.sha256)) throw new Error(`artifact ${name} SHA-256 is invalid`);
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
  sha256File,
  verifyChecksums,
};
