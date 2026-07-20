import { describe, expect, test, vi } from 'vitest';

import {
  attachJsBlockWorkerRuntime,
  createJsBlockRuntimeSession,
  createJsBlockWorkerExecutor,
  reduceJsBlockRuntimeSession
} from '../index';
import type {
  JsBlockInjectedModuleMap,
  JsBlockRunRequest,
  JsBlockWorkerRuntimeScope,
  JsBlockWorkerToHostMessage
} from '../index';

const validSource = `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render(ctx) {
    ctx.events.emit('block.rendered', { title: ctx.props.title });
    return Text({ children: ctx.props.title });
  }
});
`;

function createModules(
  overrides: JsBlockInjectedModuleMap = {}
): JsBlockInjectedModuleMap {
  return {
    '@1flowbase/block-sdk': {
      defineBlock(definition: unknown) {
        return definition;
      }
    },
    '@1flowbase/block-renderer/antd-facade': {
      Text(input: { children?: unknown; props?: { children?: unknown } }) {
        return {
          primitive: 'Text',
          props: { children: input.props?.children ?? input.children }
        };
      }
    },
    ...overrides
  };
}

function createRunRequest(
  overrides: Partial<JsBlockRunRequest> = {}
): JsBlockRunRequest {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    source: validSource,
    props: { title: 'Ready' },
    state: { count: 1 },
    contextSnapshot: {
      currentUser: { id: 'user-1', displayName: 'Ada' },
      workspace: { id: 'workspace-1', name: 'Workspace' },
      application: { id: 'app-1', name: 'Application' },
      page: { id: 'page-1', route: '/frontstage/page-1', title: 'Frontstage' },
      params: { recordId: 'record-1' },
      theme: { mode: 'light', tokens: { colorPrimary: '#1677ff' } },
      ui: { locale: 'zh-CN', density: 'comfortable' }
    },
    limits: {
      timeoutMs: 1000,
      maxRenderDepth: 8,
      maxRenderNodes: 250
    },
    ...overrides
  };
}

function withoutPhaseMessages(messages: JsBlockWorkerToHostMessage[]) {
  return messages.filter((message) => message.type !== 'phase');
}

describe('JS block worker executor', () => {
  test('posts ready on init and rendered schema on run', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({ direction: 'host_to_worker', type: 'init' });
    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });

    expect(withoutPhaseMessages(messages)).toEqual([
      { direction: 'worker_to_host', type: 'ready' },
      {
        direction: 'worker_to_host',
        type: 'event',
        requestId: 'request-1',
        name: 'block.rendered',
        payload: { title: 'Ready' }
      },
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: { primitive: 'Text', props: { children: 'Ready' } }
      }
    ]);
  });

  test('leaves schema validation to the host reducer', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules({
        '@1flowbase/block-renderer/antd-facade': {
          Text() {
            return { primitive: 'Unknown' };
          }
        }
      }),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });

    const rendered = messages.find((message) => message.type === 'rendered');
    expect(rendered).toMatchObject({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'request-1',
      schema: { primitive: 'Unknown' }
    });

    const hostState = reduceJsBlockRuntimeSession(
      reduceJsBlockRuntimeSession(createJsBlockRuntimeSession(), {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      }),
      rendered
    );

    expect(hostState.requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        error: { kind: 'schema_invalid' }
      }
    });
  });

  test('maps render failures into runtime error messages', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';

export default defineBlock({
  render() {
    throw new Error('render exploded');
  }
});
`
      })
    });

    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'request-1',
        kind: 'render_failed',
        message: 'JS block render failed: render exploded',
        errors: [
          {
            code: 'runtime_error',
            path: 'runtime.render',
            message: 'JS block render failed: render exploded'
          }
        ]
      }
    ]);
  });

  test('does not execute source that fails source policy', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const defineBlock = vi.fn((definition: unknown) => definition);
    const executor = createJsBlockWorkerExecutor({
      modules: createModules({
        '@1flowbase/block-sdk': { defineBlock }
      }),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: "window.location.href = 'https://example.test';"
      })
    });

    expect(defineBlock).not.toHaveBeenCalled();
    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'request-1',
        kind: 'source_policy_failed',
        message: 'JS block source transform failed.',
        errors: [
            {
              code: 'transform_failed',
              path: 'source.identifiers.window',
              message: "Identifier 'window' is not allowed in JS block source.",
              sourceLocation: { line: 1, column: 1 }
            }
        ]
      }
    ]);
  });

  test('posts controlled action and data effects without blocking when user code does not await them', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  async render(ctx) {
    ctx.actions.invoke('record.refresh', { id: ctx.params.recordId });
    ctx.data.query('records.list', {
      model: 'private_records',
      where: { id: ctx.params.recordId }
    });
    return Text({ children: 'Done' });
  }
});
`
      })
    });

    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'action',
        requestId: 'request-1',
        effectId: expect.any(String),
        actionId: 'record.refresh',
        payload: { id: 'record-1' }
      },
      {
        direction: 'worker_to_host',
        type: 'data',
        requestId: 'request-1',
        effectId: expect.any(String),
        queryId: 'records.list',
        params: {
          model: 'private_records',
          where: { id: 'record-1' }
        }
      },
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: { primitive: 'Text', props: { children: 'Done' } }
      }
    ]);
  });

  test('waits for host data and action effect results before rendering', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    const runPromise = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  async render(ctx) {
    const record = await ctx.data.query('records', {
      where: { id: ctx.params.recordId }
    });
    await ctx.actions.invoke('record.track', { id: record.id });
    return Text({ children: record.title });
  }
});
`
      })
    });

    await Promise.resolve();

    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'data',
        requestId: 'request-1',
        effectId: expect.any(String),
        queryId: 'records',
        params: { where: { id: 'record-1' } }
      }
    ]);
    const dataEffectId = getEffectId(messages[0]);

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId: dataEffectId,
      ok: true,
      value: { id: 'record-1', title: 'Ready' }
    });
    await Promise.resolve();

    expect(messages[1]).toEqual({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      effectId: expect.any(String),
      actionId: 'record.track',
      payload: { id: 'record-1' }
    });
    const actionEffectId = getEffectId(messages[1]);

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId: actionEffectId,
      ok: true,
      value: { tracked: true }
    });
    await runPromise;

    expect(messages[2]).toEqual({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'request-1',
      schema: { primitive: 'Text', props: { children: 'Ready' } }
    });
  });

  test('maps failed host effect results into a runtime error', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    const runPromise = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  async render(ctx) {
    await ctx.data.query('private_records');
    return Text({ children: 'Done' });
  }
});
`
      })
    });
    await Promise.resolve();

    const effectId = getEffectId(messages[0]);
    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId,
      ok: false,
      error: {
        kind: 'runtime_error',
        message: 'Query denied by host policy.',
        errors: [
          {
            code: 'query_denied',
            path: 'data.query',
            message: 'Query denied by host policy.'
          }
        ]
      }
    });
    await runPromise;

    expect(messages.at(-1)).toEqual({
      direction: 'worker_to_host',
      type: 'error',
      requestId: 'request-1',
      kind: 'effect_failed',
      message: 'JS block render failed: Query denied by host policy.',
      errors: [
        {
          code: 'runtime_error',
          path: 'runtime.render',
          message: 'JS block render failed: Query denied by host policy.'
        }
      ]
    });
  });

  test('attached runtime accepts effect results while a run is pending', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    let listener: ((event: { data: unknown }) => void) | null = null;
    const scope: JsBlockWorkerRuntimeScope = {
      postMessage: (message) => messages.push(message),
      addEventListener: (_type, nextListener) => {
        listener = nextListener;
      },
      removeEventListener: (_type, nextListener) => {
        if (listener === nextListener) {
          listener = null;
        }
      }
    };
    const attached = attachJsBlockWorkerRuntime(scope, {
      modules: createModules()
    });

    listener?.({
      data: {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest({
          source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  async render(ctx) {
    const record = await ctx.data.query('records');
    return Text({ children: record.title });
  }
});
`
        })
      }
    });
    await Promise.resolve();

    const effectId = getEffectId(messages[0]);
    listener?.({
      data: {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId,
        ok: true,
        value: { title: 'Ready' }
      }
    });

    await expect(
      Promise.race([attached.flush().then(() => 'flushed'), delay(20)])
    ).resolves.toBe('flushed');

    expect(messages.at(-1)).toEqual({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'request-1',
      schema: { primitive: 'Text', props: { children: 'Ready' } }
    });
  });

  test('dispose clears pending effects and ignores later effect results', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    const runPromise = executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  async render(ctx) {
    const record = await ctx.data.query('records');
    return Text({ children: record.title });
  }
});
`
      })
    });
    await Promise.resolve();
    const effectId = getEffectId(messages[0]);

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'dispose',
      requestId: 'request-1'
    });
    await runPromise;
    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId,
      ok: true,
      value: { title: 'Late' }
    });

    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'data',
        requestId: 'request-1',
        effectId,
        queryId: 'records'
      }
    ]);
  });

  test('applies ctx.patch only to the current run state snapshot', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });
    const request = createRunRequest({
      state: { count: 1 },
      source: `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render(ctx) {
    ctx.patch({ count: ctx.state.count + 1 });
    return Text({ children: ctx.state.count });
  }
});
`
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request
    });

    expect(request.state).toEqual({ count: 1 });
    expect(withoutPhaseMessages(messages)).toEqual([
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: { primitive: 'Text', props: { children: 2 } }
      }
    ]);
  });

  test('ignores run messages after dispose', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    const executor = createJsBlockWorkerExecutor({
      modules: createModules(),
      postMessage: (message) => messages.push(message)
    });

    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'dispose'
    });
    await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });

    expect(withoutPhaseMessages(messages)).toEqual([]);
  });

  test('attaches to a worker-like scope and detaches on dispose', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    let listener: ((event: { data: unknown }) => void) | null = null;
    const scope: JsBlockWorkerRuntimeScope = {
      postMessage: (message) => messages.push(message),
      addEventListener: (_type, nextListener) => {
        listener = nextListener;
      },
      removeEventListener: (_type, nextListener) => {
        if (listener === nextListener) {
          listener = null;
        }
      }
    };

    const attached = attachJsBlockWorkerRuntime(scope, {
      modules: createModules()
    });
    listener?.({ data: { direction: 'host_to_worker', type: 'init' } });
    await attached.flush();

    attached.dispose();
    listener?.({ data: { direction: 'host_to_worker', type: 'init' } });
    await attached.flush();

    expect(withoutPhaseMessages(messages)).toEqual([{ direction: 'worker_to_host', type: 'ready' }]);
    expect(listener).toBeNull();
  });
});

function getEffectId(message: JsBlockWorkerToHostMessage | undefined): string {
  expect(message).toMatchObject({
    effectId: expect.any(String)
  });
  return (message as { effectId: string }).effectId;
}

function delay(ms: number): Promise<'timeout'> {
  return new Promise((resolve) => {
    setTimeout(() => resolve('timeout'), ms);
  });
}
