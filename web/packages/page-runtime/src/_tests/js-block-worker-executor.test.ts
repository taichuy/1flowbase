import { describe, expect, test, vi } from 'vitest';

import {
  attachJsBlockWorkerRuntime,
  createJsBlockWorkerExecutor,
  type JsBlockRunRequest,
  type JsBlockWorkerToHostMessage
} from '../index';

function request(source: string): JsBlockRunRequest {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    source,
    inputs: {},
    props: { title: 'Ready' },
    state: {},
    contextSnapshot: {},
    limits: { timeoutMs: 1_000 }
  };
}

describe('JS block worker executor', () => {
  test('executes main and publishes logs, events and a structured completion', async () => {
    const executor = createJsBlockWorkerExecutor({ modules: {} });
    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: request(`
        async function main(ctx) {
          console.log('running', { token: 'redacted-by-host' });
          ctx.events.emit('ready', { id: 1 });
          return {
            view: { primitive: 'Text', props: { children: ctx.props.title } },
            outputs: { title: ctx.props.title }
          };
        }
        export default { main };
      `)
    });
    expect(messages).toContainEqual(
      expect.objectContaining({
        type: 'log',
        level: 'info',
        message: expect.stringContaining('running')
      })
    );
    expect(messages).toContainEqual(
      expect.objectContaining({
        type: 'event',
        name: 'ready',
        payload: { id: 1 }
      })
    );
    expect(messages).toContainEqual({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'request-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: { title: 'Ready' }
    });
  });

  test('waits for a bound interface result before main completes', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: {},
      postMessage: (message) => messages.push(message)
    });
    const pending = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: request(`
        async function main(ctx) {
          const response = await ctx.interfaces.call('listRecords', { query: { page: 1 } });
          return {
            view: { primitive: 'Text', props: { children: response.total } },
            outputs: { total: response.total }
          };
        }
        export default { main };
      `)
    });
    await vi.waitFor(() =>
      expect(messages).toContainEqual(
        expect.objectContaining({
          type: 'interface',
          effectId: expect.any(String),
          bindingAlias: 'listRecords'
        })
      )
    );
    const effect = messages.find((message) => message.type === 'interface');
    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId: (effect as { effectId: string }).effectId,
      ok: true,
      value: { total: 3 }
    });
    await pending;
    expect(messages).toContainEqual(
      expect.objectContaining({
        type: 'completed',
        outputs: { total: 3 }
      })
    );
  });

  test('maps invalid modules, main failures and invalid results to stable errors', async () => {
    const executor = createJsBlockWorkerExecutor({ modules: {} });
    for (const [source, path] of [
      ['export default {};', 'source.defaultExport'],
      [
        'async function main(){throw new Error("boom")} export default {main};',
        'runtime.main'
      ],
      [
        'async function main(){return null} export default {main};',
        'runtime.result'
      ]
    ] as const) {
      const messages = await executor.handleMessage({
        direction: 'host_to_worker',
        type: 'run',
        request: request(source)
      });
      expect(messages.at(-1)).toMatchObject({
        type: 'error',
        errors: [{ path }]
      });
    }
  });

  test('attached runtime ignores messages after dispose', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    let listener: ((event: { data: unknown }) => void) | undefined;
    const attached = attachJsBlockWorkerRuntime(
      {
        postMessage: (message) => messages.push(message),
        addEventListener: (_type, next) => {
          listener = next;
        },
        removeEventListener: () => undefined
      },
      { modules: {} }
    );
    attached.dispose();
    listener?.({ data: { direction: 'host_to_worker', type: 'init' } });
    await attached.flush();
    expect(messages).toEqual([]);
  });
});
