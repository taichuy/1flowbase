'use strict';

const crypto = require('node:crypto');

const TERMINAL_STATUSES = new Set(['succeeded', 'incomplete', 'failed', 'cancelled']);

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function unwrapData(payload) {
  return payload && typeof payload === 'object' && 'data' in payload ? payload.data : payload;
}

function sanitizeDurableRun(payload) {
  const data = unwrapData(payload);
  const errorMessage = typeof data?.error?.message === 'string' ? data.error.message : null;
  return {
    id: data?.id ?? null,
    status: data?.status ?? null,
    external_trace_id: data?.correlation?.external_trace_id ?? data?.external_trace_id ?? null,
    application_id: data?.application_id ?? null,
    provider_instance_id: data?.provider_instance_id ?? null,
    ...(errorMessage === null ? {} : { error_message: errorMessage }),
  };
}

function durableDigest(run) {
  return crypto.createHash('sha256').update(stableJson(run)).digest('hex');
}

async function queryDurableRun(target, trace, fetchImpl = globalThis.fetch) {
  if (target?.evidence_role !== 'gateway-support-target') throw new Error('durable evidence requires a Gateway target');
  if (typeof trace?.run_id !== 'string') throw new Error('durable evidence requires a decoded run id');
  const template = target.durable?.query_run;
  const url = template?.url_template?.replace('{run_id}', encodeURIComponent(trace.run_id));
  if (!url || url === template.url_template) throw new Error('Gateway durable query endpoint omitted {run_id}');
  const response = await fetchImpl(url, { method: template.method ?? 'GET', headers: template.headers ?? {} });
  if (!response.ok) throw new Error(`Responses WebSocket durable query returned HTTP ${response.status}`);
  const run = sanitizeDurableRun(await response.json());
  if (run.id !== trace.run_id) throw new Error('Responses WebSocket protocol/durable run id mismatch');
  if (!TERMINAL_STATUSES.has(run.status)) throw new Error(`Responses WebSocket durable status remained ${run.status}`);
  if (run.external_trace_id !== null && run.external_trace_id !== trace.client_trace_id) {
    throw new Error('Responses WebSocket durable/client trace id mismatch');
  }
  return {
    schema_version: '1flowbase.responses-websocket-durable/v1',
    run,
    digest_sha256: durableDigest(run),
  };
}

module.exports = { TERMINAL_STATUSES, durableDigest, queryDurableRun, sanitizeDurableRun, stableJson };
