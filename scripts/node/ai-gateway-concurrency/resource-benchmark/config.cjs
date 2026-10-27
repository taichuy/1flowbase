'use strict';
const path = require('node:path');
const { SIZES } = require('./payload.cjs');
const { UPLOAD_SIZES } = require('./mixed-workload.cjs');
const U32_MAX = 4294967295;
const CONNECTION_PARAMETERS = {
  RB_DATABASE_POOL_MAX_CONNECTIONS: 'API_DATABASE_POOL_MAX_CONNECTIONS',
  RB_DATABASE_MANAGED_MAX_CONNECTIONS: 'API_DATABASE_MANAGED_MAX_CONNECTIONS',
};
function numericConnectionPlan(plan) {
  if (plan === undefined) return {};
  if (!plan || typeof plan !== 'object' || Array.isArray(plan)) throw Error('invalid connection plan');
  const selected = {};
  for (const name of Object.values(CONNECTION_PARAMETERS)) {
    if (!Object.hasOwn(plan, name)) continue;
    const value = plan[name];
    if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > U32_MAX) throw Error(`invalid ${name}`);
    selected[name] = value;
  }
  if (Object.keys(selected).length) {
    // Existing fixture P=5; W1's unspecified managed limit inherits P. Rust also
    // checks this known allocation sum for u32 overflow without a full budget.
    const business = selected.API_DATABASE_POOL_MAX_CONNECTIONS ?? 5;
    const managed = selected.API_DATABASE_MANAGED_MAX_CONNECTIONS ?? business;
    if (business + managed > U32_MAX) throw Error('postgres connection budget overflow');
  }
  return selected;
}
function parseConnectionPlan(env) {
  const plan = {};
  for (const [input, target] of Object.entries(CONNECTION_PARAMETERS)) {
    const raw = env[input];
    if (raw === undefined) continue;
    // Match Rust's positive u32 grammar; Number alone also accepts blanks,
    // fractions written as 5.0, exponent notation, hex and booleans.
    if (typeof raw !== 'string' || !/^\+?[0-9]+$/.test(raw)) throw Error(`invalid ${input}`);
    const value = Number(raw);
    if (!Number.isInteger(value) || value < 1 || value > U32_MAX) throw Error(`invalid ${input}`);
    plan[target] = value;
  }
  return numericConnectionPlan(plan);
}
function connectionEnvironment(config) {
  return Object.fromEntries(Object.entries(numericConnectionPlan(config.connectionPlan)).map(([name, value]) => [name, String(value)]));
}
function integer(env, name, fallback, low, high) {
  const value = Number(env[name] ?? fallback);
  if (!Number.isInteger(value) || value < low || value > high) throw Error(`invalid ${name}`);
  return value;
}
function configuration(env = process.env) {
  const required = name => { if (!env[name]) throw Error(`missing ${name}`); return env[name]; };
  const repoRoot = path.resolve(required('RB_REPO_ROOT'));
  const historyBytes = integer(env, 'RB_HISTORY_BYTES', 8192, 8192, 524288);
  const workload = env.RB_WORKLOAD ?? 'text';
  if (!['text', 'mixed'].includes(workload)) throw Error('invalid RB_WORKLOAD');
  const uploadBytes = integer(env, 'RB_UPLOAD_BYTES', UPLOAD_SIZES[0], UPLOAD_SIZES[0], UPLOAD_SIZES[1]);
  if (!UPLOAD_SIZES.includes(uploadBytes)) throw Error('unsupported RB_UPLOAD_BYTES');
  const concurrency = integer(env, 'RB_CONCURRENCY', 1, 1, 8);
  if (!SIZES.includes(historyBytes) || ![1, 3, 8].includes(concurrency)) throw Error('unsupported workload dimension');
  const connectionPlan = parseConnectionPlan(env);
  if (env.RB_DEDICATED_DATABASE !== '1') throw Error('RB_DEDICATED_DATABASE=1 acknowledgment required');
  const apiPort = env.RB_API_PORT ? integer(env, 'RB_API_PORT', 0, 1, 65535) : null;
  if ([7600, 7800].includes(apiPort)) throw Error('protected API port');
  const artifactRoot = path.resolve(required('RB_ARTIFACT_ROOT'));
  const governance = path.join(repoRoot, 'tmp', 'test-governance');
  if (!artifactRoot.startsWith(`${governance}${path.sep}`)) throw Error('artifact root must be a child of repoRoot/tmp/test-governance');
  return {
    repoRoot, artifactRoot, apiPort, historyBytes, concurrency, workload, uploadBytes,
    ...(Object.keys(connectionPlan).length ? { connectionPlan } : {}),
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
  const result = Object.fromEntries(['historyBytes', 'concurrency', 'workload', 'uploadBytes', 'warmups', 'rounds', 'idleMs', 'drainMs', 'sampleMs', 'dedicatedCluster'].map(key => [key, config[key]]));
  const plan = numericConnectionPlan(config.connectionPlan);
  if (Object.keys(plan).length) result.connectionPlan = plan;
  return result;
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
module.exports = { configuration, publicConfiguration, connectionEnvironment, secrets, safeError };
