'use strict';
const fs = require('node:fs');
const path = require('node:path');
const Module = require('node:module');

// Re-evaluate the frozen module with only its input inventory narrowed.
// Its client, durable queries and all correctness assertions are unchanged.
function loadSingleErrorRow(repoRoot) {
  const filename = path.join(repoRoot, 'scripts/node/ai-gateway-concurrency/responses-websocket-acceptance/error-matrix.js');
  const scoped = new Module(filename, module);
  scoped.filename = filename;
  scoped.paths = Module._nodeModulePaths(path.dirname(filename));
  const originalRequire = scoped.require.bind(scoped);
  scoped.require = (name) => {
    const value = originalRequire(name);
    if (name !== '../protocol-oracle/error-fidelity') return value;
    const fixtures = value.UPSTREAM_ERROR_FIXTURES.filter(row => row.id === 'json');
    if (fixtures.length !== 1) throw new Error('Frozen JSON error fixture unavailable');
    return { ...value, UPSTREAM_ERROR_FIXTURES: fixtures, ERROR_SURFACES: ['responses-sse'] };
  };
  scoped._compile(fs.readFileSync(filename, 'utf8'), filename);
  return scoped.exports;
}

function tracedFetch(originalFetch, records, redact, now = () => process.hrtime.bigint().toString()) {
  return async (url, options = {}) => {
    const parsed = new URL(String(url));
    const row = { method: options.method || 'GET', url: `${parsed.origin}${parsed.pathname}`, start_ns: now() };
    if (options.body && typeof options.body === 'string') {
      try { row.has_previous_response_id = Boolean(JSON.parse(options.body).previous_response_id); } catch {}
    }
    records.push(row);
    try {
      const response = await originalFetch(url, options);
      row.headers_ns = now();
      row.status = response.status;
      if (!response.ok) {
        // Clone only unsuccessful responses; never tee or consume a success SSE stream.
        row.error_body = redact((await response.clone().text()).slice(0, 4096));
        row.error_body_ns = now();
      }
      return response;
    } catch (error) {
      row.error = redact(error.message);
      row.error_ns = now();
      throw error;
    }
  };
}
module.exports = { loadSingleErrorRow, tracedFetch };
