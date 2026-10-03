'use strict';
const path = require('node:path');
const { SIZES } = require('./payload.cjs');
function integer(env, name, fallback, low, high) {
  const value = Number(env[name] ?? fallback);
  if (!Number.isInteger(value) || value < low || value > high) throw Error(`invalid ${name}`);
  return value;
}
function configuration(env = process.env) {
  const required = name => { if (!env[name]) throw Error(`missing ${name}`); return env[name]; };
  const repoRoot = path.resolve(required('RB_REPO_ROOT'));
  const historyBytes = integer(env, 'RB_HISTORY_BYTES', 8192, 8192, 524288);
  const concurrency = integer(env, 'RB_CONCURRENCY', 1, 1, 8);
  if (!SIZES.includes(historyBytes) || ![1, 8].includes(concurrency)) throw Error('unsupported workload dimension');
  if (env.RB_DEDICATED_DATABASE !== '1') throw Error('RB_DEDICATED_DATABASE=1 acknowledgment required');
  const apiPort = env.RB_API_PORT ? integer(env, 'RB_API_PORT', 0, 1, 65535) : null;
  if ([7600, 7800].includes(apiPort)) throw Error('protected API port');
  const artifactRoot = path.resolve(required('RB_ARTIFACT_ROOT'));
  const governance = path.join(repoRoot, 'tmp', 'test-governance');
  if (!artifactRoot.startsWith(`${governance}${path.sep}`)) throw Error('artifact root must be a child of repoRoot/tmp/test-governance');
  return {
    repoRoot, artifactRoot, apiPort, historyBytes, concurrency,
    apiServerBin: path.resolve(required('RB_API_BINARY')),
    openaiPackage: path.resolve(required('RB_OPENAI_PACKAGE')),
    anthropicPackage: path.resolve(required('RB_ANTHROPIC_PACKAGE')),
    openaiCompatiblePackage: path.resolve(required('RB_OPENAI_COMPATIBLE_PACKAGE')),
    databaseUrl: required('RB_DATABASE_URL'),
    pgModule: env.RB_PG_MODULE || null,
    dedicatedCluster: env.RB_DEDICATED_CLUSTER === '1',
    databasePid: env.RB_DATABASE_PID ? integer(env, 'RB_DATABASE_PID', 0, 1, 2147483647) : null,
    warmups: integer(env, 'RB_WARMUPS', 1, 0, 5),
    rounds: integer(env, 'RB_ROUNDS', 3, 1, 10),
    idleMs: integer(env, 'RB_IDLE_MS', 3000, 500, 10000),
    drainMs: integer(env, 'RB_DRAIN_MS', 3000, 500, 10000),
    sampleMs: integer(env, 'RB_SAMPLE_MS', 100, 50, 1000),
  };
}
// Deliberate whitelist: URLs, env, owner sessions and fixture.result must never be serialized.
function publicConfiguration(config) {
  return Object.fromEntries(['historyBytes', 'concurrency', 'warmups', 'rounds', 'idleMs', 'drainMs', 'sampleMs', 'dedicatedCluster'].map(key => [key, config[key]]));
}
function secrets(config) {
  let password = '';
  try { password = decodeURIComponent(new URL(config.databaseUrl).password); } catch { /* invalid input */ }
  return [config.databaseUrl, password].filter(Boolean);
}
function safeError(error, canaries = []) {
  const scrub = value => {
    let text = String(value ?? '');
    for (const secret of canaries.filter(Boolean).sort((a,b) => b.length - a.length)) text = text.split(secret).join('[redacted]');
    return text.replace(/postgres(?:ql)?:\/\/[^\s]+/gi, '[redacted-postgres-url]').slice(0, 1500);
  };
  return { name: scrub(error?.name || 'Error'), code: error?.code === undefined ? null : scrub(error.code), message: scrub(error?.message || 'unknown error') };
}
module.exports = { configuration, publicConfiguration, secrets, safeError };
