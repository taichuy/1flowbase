import { describe, expect, test, vi } from 'vitest';

import {
  attachJsBlockWorkerRuntime,
  compileAndTransformJsBlockSource,
  createCompiledBlockArtifact,
  createJsBlockWorkerExecutor,
  type JsBlockRunRequest,
  type JsBlockWorkerToHostMessage
} from '../index';

const runtimeFingerprint = 'fixture-runtime';

function compiledRequest(source: string, fallbackSource = source): JsBlockRunRequest {
  const transformed = compileAndTransformJsBlockSource(source);
  if (!transformed.ok) throw new Error('fixture transform failed');
  const artifact = createCompiledBlockArtifact({
    source,
    runtimeFingerprint,
    allowedImports: [],
    transformed
  });
  return {
    ...request(source),
    program: {
      kind: 'compiled_artifact',
      artifact,
      sourceSha256: artifact.sourceSha256,
      fallback: { kind: 'source', source: fallbackSource }
    }
  };
}

function request(source: string): JsBlockRunRequest {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    program: { kind: 'source', source },
    inputs: {},
    props: { title: 'Ready' },
    state: {},
    contextSnapshot: {},
    limits: { timeoutMs: 1_000 }
  };
}

describe('JS block worker executor', () => {
  test('AC-023 D5-005 executes an artifact without reading an invalid fallback source', async () => {
    const executor = createJsBlockWorkerExecutor({ modules: {} });
    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: compiledRequest(
        `async function main(){return {view:{primitive:'Text',props:{children:'artifact'}},outputs:{hit:true}}} export default {main};`,
        'this is deliberately not valid source {'
      )
    });
    expect(messages).toContainEqual(
      expect.objectContaining({ type: 'phase', phase: 'executing' })
    );
    expect(messages).not.toContainEqual(
      expect.objectContaining({ type: 'phase', phase: 'compiling' })
    );
    expect(messages.at(-1)).toMatchObject({
      type: 'completed',
      outputs: { hit: true }
    });
  });

  test('AC-024 runs main and effects again for every new Worker request', async () => {
    const source = `async function main(ctx){ctx.events.emit('ran');return {view:{primitive:'Text',props:{}},outputs:{}}} export default {main};`;
    const first = await createJsBlockWorkerExecutor({ modules: {} }).handleMessage({ direction: 'host_to_worker', type: 'run', request: compiledRequest(source) });
    const second = await createJsBlockWorkerExecutor({ modules: {} }).handleMessage({ direction: 'host_to_worker', type: 'run', request: { ...compiledRequest(source), requestId: 'request-2' } });
    expect(first.filter((message) => message.type === 'event')).toHaveLength(1);
    expect(second.filter((message) => message.type === 'event')).toHaveLength(1);
  });
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

  test('waits for a source-described interface result before main completes', async () => {
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
          const response = await ctx.api.get('/api/console/test', { query: { page: 1 } });
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
          method: 'GET',
          path: '/api/console/test'
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

  test('fails locally instead of posting a non-canonical API route', async () => {
    const executor = createJsBlockWorkerExecutor({ modules: {} });
    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: request(`
        async function main(ctx) {
          await ctx.api.get('https://example.com/private');
          return { view: { primitive: 'Text', props: {} }, outputs: {} };
        }
        export default { main };
      `)
    });

    expect(messages.some((message) => message.type === 'interface')).toBe(
      false
    );
    expect(messages.at(-1)).toMatchObject({
      type: 'error',
      kind: 'main_failed',
      message: expect.stringContaining(
        'API path must be a canonical relative path template.'
      )
    });
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
