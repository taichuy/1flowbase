'use strict';

const fs = require('node:fs');
const path = require('node:path');

const LOCK_SCHEMA = '1flowbase.ai-gateway-client-compatibility-lock/v1';
const DEFAULT_LOCK_PATH = path.join(__dirname, 'client-compatibility.lock.json');
const CLIENT_NAMES = Object.freeze(['claude', 'codex', 'opencode']);
const GATEWAY_PROTOCOLS = Object.freeze({
  claude: 'anthropic-messages',
  codex: 'openai-responses',
  opencode: 'openai-chat-completions',
});

function requiredObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} is required`);
  return value;
}

function packageSpec(value, label) {
  requiredObject(value, label);
  if (typeof value.name !== 'string' || value.name.length === 0) throw new Error(`${label} name is required`);
  if (typeof value.version !== 'string' || !/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/u.test(value.version)) {
    throw new Error(`${label} version must be exact`);
  }
  if (typeof value.integrity !== 'string' || !/^sha512-[A-Za-z0-9+/]+={0,2}$/u.test(value.integrity)) {
    throw new Error(`${label} integrity must be SHA-512`);
  }
}

function relativeExecutable(value, label) {
  if (typeof value !== 'string' || value.length === 0 || path.isAbsolute(value) || value.includes('..')) {
    throw new Error(`${label} must be a runtime-relative path`);
  }
}

function validateLock(lock) {
  requiredObject(lock, 'client compatibility lock');
  if (lock.schema_version !== LOCK_SCHEMA) throw new Error('client compatibility lock schema mismatch');
  if (lock.platform !== 'linux-x64') throw new Error('blocking client compatibility platform must be linux-x64');
  if (lock.node_major !== 24) throw new Error('blocking client compatibility Node major must be 24');
  const official = requiredObject(lock.official_plugins, 'official plugin pin');
  if (official.repository !== 'taichuy/1flowbase-official-plugins') throw new Error('official plugin repository mismatch');
  if (!/^[a-f0-9]{40}$/u.test(official.revision)) throw new Error('official plugin revision must be a full SHA');

  const packages = requiredObject(lock.packages, 'package inventory');
  for (const [name, value] of Object.entries(packages)) packageSpec(value, `package ${name}`);
  const clients = requiredObject(lock.clients, 'client inventory');
  if (Object.keys(clients).sort().join(',') !== [...CLIENT_NAMES].sort().join(',')) {
    throw new Error('client inventory must contain exactly claude, codex, and opencode');
  }
  for (const name of CLIENT_NAMES) {
    const client = requiredObject(clients[name], `client ${name}`);
    if (!packages[client.package]) throw new Error(`client ${name} package is not pinned`);
    if (!packages[client.platform_package]) throw new Error(`client ${name} platform package is not pinned`);
    if (client.adapter_package !== null && !packages[client.adapter_package]) {
      throw new Error(`client ${name} adapter package is not pinned`);
    }
    relativeExecutable(client.executable, `client ${name} executable`);
    relativeExecutable(client.adapter_executable, `client ${name} adapter executable`);
    if (client.gateway_protocol !== GATEWAY_PROTOCOLS[name]) {
      throw new Error(`client ${name} gateway protocol mismatch`);
    }
    if (name !== 'opencode' && typeof client.binding_env !== 'string') {
      throw new Error(`client ${name} must bind its real executable`);
    }
  }
  return lock;
}

function loadLock(filePath = DEFAULT_LOCK_PATH) {
  const resolved = path.resolve(filePath);
  let lock;
  try {
    lock = JSON.parse(fs.readFileSync(resolved, 'utf8'));
  } catch (error) {
    throw new Error(`client compatibility lock is unavailable: ${error.message}`);
  }
  return validateLock(lock);
}

module.exports = {
  CLIENT_NAMES,
  DEFAULT_LOCK_PATH,
  GATEWAY_PROTOCOLS,
  LOCK_SCHEMA,
  loadLock,
  validateLock,
};
