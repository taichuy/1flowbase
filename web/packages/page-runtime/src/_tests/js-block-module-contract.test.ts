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
    params: {},
    props: {},
    state: {},
    patch() {},
    interfaces: { call: vi.fn() },
    events: { emit: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: { locale: 'en_US' }
  };
}

function createRequest(source: string): JsBlockRunRequest {
  return {
    requestId: 'run-1',
    blockId: 'block-1',
    source,
    inputs: { title: 'Conversations' },
    props: {},
    state: {},
    contextSnapshot: {
      workspace: { id: 'workspace-1' },
      application: { id: 'application-1' },
      page: { id: 'page-1', route: '/demo' }
    },
    limits: { timeoutMs: 1_000 },
    allowedImports: ['@1flowbase/block-sdk']
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

  test('AC-004 waits for a bound interface effect before completing main', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: {},
      postMessage: (message) => messages.push(message)
    });
    const request = createRequest(`
      async function main(ctx) {
        const response = await ctx.interfaces.call('listConversations', {
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
          bindingAlias: 'listConversations',
          request: { query: { page: 1 } }
        })
      );
    });
    const effect = messages.find(
      (message) => message.type === 'interface'
    );
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

  test('AC-004 mediator accepts only explicitly bound interface aliases', () => {
    const mediator = createBlockContextMediator({
      allowedInterfaces: ['listConversations']
    });

    expect(
      mediator.handle({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-1',
        bindingAlias: 'listConversations',
        request: { query: { page: 1 } }
      }).result
    ).toMatchObject({ ok: true });
    expect(
      mediator.handle({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-2',
        bindingAlias: 'deleteEverything'
      }).result
    ).toMatchObject({
      ok: false,
      code: 'interface_denied',
      path: 'interface.bindingAlias'
    });
  });
});
