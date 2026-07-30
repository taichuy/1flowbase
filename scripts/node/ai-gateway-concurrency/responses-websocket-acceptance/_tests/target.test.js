'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  RESPONSES_WEBSOCKET_BETA,
  createDirectUpstreamProbe,
  createGatewayTarget,
  publicTarget,
} = require('../target');

function readyManifest() {
  return {
    targets: {
      openai: {
        application_id: 'application-1',
        provider_instance_id: 'provider-1',
        model: 'published-model',
        upstream_model: 'upstream-model',
        api_key: 'application-secret',
        gateway: { responses_url: 'http://127.0.0.1:4100/v1/responses' },
        durable: {
          query_run: {
            method: 'GET',
            url_template: 'http://127.0.0.1:4100/api/agent/v1/runs/{run_id}',
            headers: { authorization: 'Bearer application-secret' },
          },
          list_runs: { url: 'http://127.0.0.1:4100/api/console/applications/application-1/logs/runs' },
        },
      },
    },
    controlled_upstream: { snapshot_url: 'http://127.0.0.1:4000/__control/snapshot' },
  };
}

test('Root #1461 AC WebSocket target uses Gateway URL, key, model, and durable endpoints', () => {
  const target = createGatewayTarget(readyManifest());
  assert.equal(target.evidence_role, 'gateway-support-target');
  assert.equal(target.url, 'ws://127.0.0.1:4100/v1/responses');
  assert.equal(target.model, 'published-model');
  assert.equal(target.upstream_model, 'upstream-model');
  assert.equal(publicTarget(target).upstream_model, 'upstream-model');
  assert.equal(target.connect_headers.authorization, 'Bearer application-secret');
  assert.equal(target.connect_headers['openai-beta'], RESPONSES_WEBSOCKET_BETA);
  assert.match(target.durable.query_run.url_template, /\{run_id\}/u);
  assert.equal(JSON.stringify(publicTarget(target)).includes('application-secret'), false);
});

test('Root #1461 controlled negative: direct mock is probe-only and cannot become Gateway evidence', () => {
  const probe = createDirectUpstreamProbe('ws://127.0.0.1:4000');
  assert.equal(probe.evidence_role, 'upstream-probe-only');
  assert.throws(() => publicTarget(probe), /requires a Gateway target/u);
});
