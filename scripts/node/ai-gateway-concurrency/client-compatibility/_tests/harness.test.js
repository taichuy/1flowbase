'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const { runAcpClient } = require('../harness');

test('D7-AC-002/003/004/005: ACP harness drives auth, one session, tool permission, and a new prompt', async () => {
  const released = [];
  const result = await runAcpClient({
    name: 'fixture',
    command: process.execPath,
    args: [path.join(__dirname, 'fixtures/fake-acp-agent.js')],
    cwd: process.cwd(),
    env: process.env,
    auth: { methodId: 'gateway', _meta: { gateway: { baseUrl: 'http://127.0.0.1', headers: { authorization: 'secret' } } } },
  }, {
    prompts: ['tool prompt', 'new prompt'],
    releaseOnMarkers: ['marker-1'],
    onMarker: async (marker) => released.push(marker),
  });
  assert.deepEqual(released, ['marker-1']);
  assert.equal(result.turns.length, 2);
  assert.equal(result.turns.every((turn) => turn.stop_reason === 'end_turn'), true);
  assert.equal(result.tools.some((tool) => tool.status === 'completed'), true);
  assert.match(result.text, /marker-after-tool/u);
  const events = result.timeline.map((entry) => entry.event);
  assert.ok(events.indexOf('barrier_released') < events.lastIndexOf('prompt_terminal'));
  assert.equal(JSON.stringify(result.timeline).includes('secret'), false);
});

test('D7-AC-004 controlled negative: permission without an allow option cancels', () => {
  const { permissionResponse } = require('../harness');
  assert.deepEqual(permissionResponse({ options: [] }), { outcome: { outcome: 'cancelled' } });
});

test('D7-AC-002 controlled negative: orphan ACP responses fail the lifecycle', async () => {
  await assert.rejects(runAcpClient({
    name: 'fixture',
    command: process.execPath,
    args: [path.join(__dirname, 'fixtures/fake-acp-orphan.js')],
    cwd: process.cwd(),
    env: process.env,
    auth: null,
  }, { prompts: ['prompt'], timeoutMs: 2000 }), /orphan response/u);
});
