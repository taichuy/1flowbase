'use strict';
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');
const { secrets, safeError } = require('./config.cjs');
const execute = promisify(execFile);
const SIZE_SQL = `SELECT pg_database_size(current_database())::text AS database_bytes,
  coalesce(sum(pg_relation_size(c.oid)),0)::text AS heap_bytes,
  coalesce(sum(pg_indexes_size(c.oid)),0)::text AS index_bytes,
  coalesce(sum(CASE WHEN c.reltoastrelid = 0 THEN 0 ELSE pg_total_relation_size(c.reltoastrelid) END),0)::text AS toast_bytes
  FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
  WHERE c.relkind IN ('r','m') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%'`;
function psqlEnvironment(databaseUrl) {
  const url = new URL(databaseUrl);
  return {
    PATH: process.env.PATH || '/usr/bin:/bin', LC_ALL: 'C',
    PGHOST: url.hostname.replace(/^\[|\]$/g, ''), PGPORT: url.port || '5432',
    PGDATABASE: decodeURIComponent(url.pathname.slice(1)), PGUSER: decodeURIComponent(url.username),
    PGPASSWORD: decodeURIComponent(url.password), PGCONNECT_TIMEOUT: '3',
    PGAPPNAME: '1flowbase-resource-observer', PGOPTIONS: '-c statement_timeout=3000',
    ...(url.searchParams.has('sslmode') ? { PGSSLMODE: url.searchParams.get('sslmode') } : {}),
  };
}
async function psqlQuery(databaseUrl, sql) {
  // Fixed observation SQL only. Credentials and even database name are exclusively PG* env.
  const { stdout } = await execute('/usr/bin/psql', ['-X', '-A', '-t', '-w', '-v', 'ON_ERROR_STOP=1', '-c', `SELECT row_to_json(observation)::text FROM (${sql}) observation`], {
    env: psqlEnvironment(databaseUrl), timeout: 6000, maxBuffer: 128 * 1024,
  });
  return JSON.parse(stdout.trim());
}
async function databaseSnapshot(config) {
  const missing = { available: false, database_bytes: null, heap_bytes: null, index_bytes: null, toast_bytes: null, wal_bytes: null };
  let client;
  let backend = 'psql';
  try {
    let Client;
    try { ({ Client } = require(config.pgModule || 'pg')); } catch { /* Use already installed psql; never install. */ }
    let query;
    if (Client) {
      backend = 'pg';
      client = new Client({ connectionString: config.databaseUrl, connectionTimeoutMillis: 3000, query_timeout: 3000 });
      await client.connect();
      query = async sql => (await client.query(sql)).rows[0];
    } else query = sql => psqlQuery(config.databaseUrl, sql);
    const row = Object.fromEntries(Object.entries(await query(SIZE_SQL)).map(([k,v]) => [k, Number(v)]));
    let wal_bytes = null, wal_error = null;
    if (config.dedicatedCluster) {
      try { wal_bytes = Number((await query("SELECT pg_wal_lsn_diff(pg_current_wal_lsn(),'0/0')::text AS bytes")).bytes); }
      catch (error) { wal_error = safeError(error, secrets(config)); }
    }
    return { available: true, observer_backend: backend, ...row, wal_bytes, wal_error, wal_scope: config.dedicatedCluster ? 'dedicated_cluster' : 'disabled_shared_cluster' };
  } catch (error) { return { ...missing, observer_backend: backend, reason: 'database_observation_unavailable', error: safeError(error, secrets(config)) }; }
  finally { if (client) await client.end().catch(() => {}); }
}
module.exports = { databaseSnapshot, psqlEnvironment };
