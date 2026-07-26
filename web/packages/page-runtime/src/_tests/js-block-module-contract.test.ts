import { describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '@1flowbase/page-protocol';

import {
  createBlockContextMediator,
  createJsBlockWorkerExecutor,
  evaluateJsBlockSource,
  runJsBlockSource,
  type JsBlockRunRequest,
  type JsBlockWorkerToHostMessage
} from '../index';

const MODULE_SOURCE = `
import type { BlockModule } from '@1flowbase/block-sdk';

async function main(ctx) {
  return {
    view: {
      primitive: 'Text',
      props: { children: ctx.inputs.title ?? 'Ready' }
    },
    outputs: { title: ctx.inputs.title ?? 'Ready' }
  };
}

export default { main } satisfies BlockModule;
`;

function createContext(): BlockContext {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1' },
    application: { id: 'application-1' },
    page: { id: 'page-1', route: '/demo' },
    inputs: { title: 'Conversations' },
    outputs: { publish: vi.fn() },
    params: {},
    props: {},
    state: {},
    patch() {},
    api: {
      get: vi.fn(),
      post: vi.fn(),
      put: vi.fn(),
      patch: vi.fn(),
      delete: vi.fn(),
      head: vi.fn(),
      options: vi.fn(),
      stream: vi.fn()
    },
    events: { emit: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: { locale: 'en_US' }
  };
}

function createRequest(source: string): JsBlockRunRequest {
  return {
    requestId: 'run-1',
    blockId: 'block-1',
    program: {
      kind: 'source',
      source,
      allowedImports: ['@1flowbase/block-sdk']
    },
    inputs: { title: 'Conversations' },
    props: {},
    state: {},
    contextSnapshot: {
      workspace: { id: 'workspace-1' },
      application: { id: 'application-1' },
      page: { id: 'page-1', route: '/demo' }
    },
    limits: { timeoutMs: 1_000 }
  };
}

describe('BlockModule runtime contract', () => {
  test('AC-001 executes main and returns view plus outputs', async () => {
    await expect(
      runJsBlockSource({
        source: MODULE_SOURCE,
        modules: {},
        context: createContext()
      })
    ).resolves.toMatchObject({
      ok: true,
      result: {
        view: {
          primitive: 'Text',
          props: { children: 'Conversations' }
        },
        outputs: { title: 'Conversations' }
      }
    });
  });

  test('AC-001 rejects the removed render contract', () => {
    const evaluation = evaluateJsBlockSource({
      source: `export default { render() { return { primitive: 'Text' }; } };`,
      modules: {}
    });

    expect(evaluation).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ path: 'source.defaultExport' }]
      }
    });
  });

  test('AC-020 waits for a source-described interface effect before completing main', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: {},
      postMessage: (message) => messages.push(message)
    });
    const request = createRequest(`
      async function main(ctx) {
        const response = await ctx.api.get('/api/console/test', {
          query: { page: 1 }
        });
        return {
          view: { primitive: 'Text', props: { children: response.total } },
          outputs: { total: response.total }
        };
      }
      export default { main };
    `);

    const pendingRun = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request
    });
    await vi.waitFor(() => {
      expect(messages).toContainEqual(
        expect.objectContaining({
          type: 'interface',
          method: 'GET',
          path: '/api/console/test',
          request: { query: { page: 1 } }
        })
      );
    });
    const effect = messages.find((message) => message.type === 'interface');
    expect(effect).toMatchObject({ effectId: expect.any(String) });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: request.requestId,
      effectId: (effect as { effectId: string }).effectId,
      ok: true,
      value: { total: 2 }
    });
    await pendingRun;

    expect(messages).toContainEqual({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'run-1',
      view: { primitive: 'Text', props: { children: 2 } },
      outputs: { total: 2 }
    });
  });

  test('AC-020 mediator accepts complete source routes and rejects incomplete ones', () => {
    const mediator = createBlockContextMediator({});

    expect(
      mediator.handle({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-1',
        method: 'GET',
        path: '/api/console/test',
        request: { query: { page: 1 } }
      }).result
    ).toMatchObject({ ok: true });
    expect(
      mediator.handle({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-2',
        method: 'GET'
      }).result
    ).toMatchObject({
      ok: false,
      code: 'effect_invalid',
      path: 'effect.path'
    });
  });

  test('pulls and cancels interface streams without buffering events in the worker', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: {},
      postMessage: (message) => messages.push(message)
    });
    const request = createRequest(`
      async function main(ctx) {
        let progress = 0;
        for await (const event of ctx.api.stream('GET', '/api/console/test')) {
          progress = event.progress;
          break;
        }
        return {
          view: { primitive: 'Text', props: { children: progress } },
          outputs: { progress }
        };
      }
      export default { main };
    `);
    const pendingRun = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request
    });
    let resolvedEffectCount = 0;
    const resolveNextEffect = async (value: unknown) => {
      await vi.waitFor(() => {
        expect(
          messages.filter((message) => message.type === 'interface').length
        ).toBeGreaterThan(resolvedEffectCount);
      });
      const effect = messages.filter((message) => message.type === 'interface')[
        resolvedEffectCount
      ] as { effectId: string };
      resolvedEffectCount += 1;
      await executor.handleMessage({
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: request.requestId,
        effectId: effect.effectId,
        ok: true,
        value
      });
    };

    await resolveNextEffect({ stream_id: 'stream-1' });
    await resolveNextEffect({ done: false, value: { progress: 50 } });
    await resolveNextEffect(undefined);
    await pendingRun;

    expect(messages.filter((message) => message.type === 'interface')).toEqual([
      expect.objectContaining({ operation: 'stream_open' }),
      expect.objectContaining({
        operation: 'stream_next',
        streamId: 'stream-1'
      }),
      expect.objectContaining({
        operation: 'stream_cancel',
        streamId: 'stream-1'
      })
    ]);
  });
});
