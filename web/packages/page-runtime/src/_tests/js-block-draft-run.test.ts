import { describe, expect, test } from 'vitest';

import {
  createJsBlockDraftRun,
  createJsBlockRuntimeSession,
  reduceJsBlockRuntimeSession,
  runJsBlockSource
} from '../index';
import type { BlockContext } from '@1flowbase/page-protocol';

const request = {
  requestId: 'run-1',
  blockId: 'block-1',
  source: 'export default { main };',
  inputs: {},
  props: {},
  state: {},
  contextSnapshot: { pageId: 'page-1' },
  limits: { timeoutMs: 1_000 }
};

describe('JS block Draft Run', () => {
  test('AC-008 aggregates one run id across view, outputs, logs and interface calls', () => {
    let state = createJsBlockRuntimeSession();
    state = reduceJsBlockRuntimeSession(state, {
      direction: 'host_to_worker',
      type: 'run',
      request
    });
    state = reduceJsBlockRuntimeSession(state, {
      direction: 'worker_to_host',
      type: 'log',
      requestId: 'run-1',
      level: 'info',
      message: 'ready'
    });
    state = reduceJsBlockRuntimeSession(state, {
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'run-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: { total: 2 }
    });
    expect(
      createJsBlockDraftRun({
        state,
        requestId: 'run-1',
        interfaceCalls: [
          {
            requestId: 'run-1',
            effectId: 'effect-1',
            interfaceId: 'list_records',
            schemaDigest: 'digest-1',
            status: 'succeeded',
            durationMs: 12,
            response: { total: 2 }
          }
        ]
      })
    ).toMatchObject({
      run_id: 'run-1',
      status: 'succeeded',
      outputs: { total: 2 },
      logs: [{ message: 'ready' }],
      interface_calls: [{ interfaceId: 'list_records' }]
    });
  });

  test('bounds and redacts worker log values', () => {
    let state = createJsBlockRuntimeSession();
    state = reduceJsBlockRuntimeSession(state, {
      direction: 'host_to_worker',
      type: 'run',
      request
    });
    const circular: Record<string, unknown> = { api_key: 'secret' };
    circular.self = circular;
    for (let index = 0; index < 205; index += 1) {
      state = reduceJsBlockRuntimeSession(state, {
        direction: 'worker_to_host',
        type: 'log',
        requestId: 'run-1',
        level: 'info',
        message: 'x'.repeat(5_000),
        data: circular
      });
    }
    const logs = state.requests['run-1'].logs;
    expect(logs).toHaveLength(200);
    expect(logs[0].message).toHaveLength(4_000);
    expect(logs[0].data).toEqual({ api_key: '[REDACTED]', self: '[Circular]' });
  });

  test('AC-008 maps a runtime failure back to the original TSX line', async () => {
    const source = [
      'async function main() {',
      '  const value: number = 1;',
      "  throw new Error('boom');",
      '}',
      'export default { main };'
    ].join('\n');
    const result = await runJsBlockSource({
      source,
      modules: {},
      context: {} as BlockContext
    });
    expect(result).toMatchObject({
      ok: false,
      error: { errors: [{ sourceLocation: { line: 3 } }] }
    });
  });
});
