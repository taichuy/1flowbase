import { describe, expect, test, vi } from 'vitest';

import {
  createCompiledBlockRuntimeFingerprint,
  JS_BLOCK_ALLOWED_IMPORTS,
  validateJsBlockSource,
  type JsBlockRunRequest,
  type JsBlockWorkerRuntimeScope,
  type JsBlockWorkerToHostMessage
} from '@1flowbase/page-runtime';

import { attachFrontstageRestrictedBlockWorkerRuntime } from '../../lib/restricted-block-worker-runtime';
import {
  createFrontstageRestrictedBlockWorkerFactory,
  getFrontstageRestrictedBlockRuntimeFingerprint,
  getFrontstageRestrictedBlockWorkerOptions,
  getFrontstageRestrictedBlockWorkerUrl
} from '../../lib/restricted-block-worker-factory';

const validSource = `
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main(ctx) {
  const record = await ctx.api.get('/api/console/test', { query: { limit: 1 } });
  ctx.events.emit('record.loaded', { title: record.title });
  return { view: Text({ children: record.title }), outputs: { title: record.title } };
}
export default { main };
`;

class FakeWorkerScope implements JsBlockWorkerRuntimeScope {
  readonly messages: JsBlockWorkerToHostMessage[] = [];
  private listener: ((event: { data: unknown }) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;

  postMessage(message: JsBlockWorkerToHostMessage): void {
    this.messages.push(message);
  }

  addEventListener(
    _type: 'message',
    listener: (event: { data: unknown }) => void
  ): void {
    this.listener = listener;
  }

  removeEventListener(
    _type: 'message',
    listener: (event: { data: unknown }) => void
  ): void {
    if (this.listener === listener) {
      this.listener = null;
    }
  }

  emit(data: unknown): void {
    this.listener?.({ data });
    this.onmessage?.({ data });
  }
}

class FakeNativeWorker {
  static readonly instances: FakeNativeWorker[] = [];

  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  onmessageerror: ((event: MessageEvent) => void) | null = null;
  readonly scriptUrl: string | URL;
  readonly options?: WorkerOptions;
  readonly messages: unknown[] = [];
  terminateCount = 0;

  constructor(scriptUrl: string | URL, options?: WorkerOptions) {
    this.scriptUrl = scriptUrl;
    this.options = options;
    FakeNativeWorker.instances.push(this);
  }

  postMessage(message: unknown): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCount += 1;
  }
}

function createRunRequest(
  overrides: Partial<JsBlockRunRequest> = {}
): JsBlockRunRequest {
  return {
    requestId: 'restricted-block:block-1:code-1',
    blockId: 'block-1',
    program: { kind: 'source', source: validSource },
    props: {},
    state: {},
    contextSnapshot: {
      page: { id: 'page-1', route: '/frontstage/page-1' }
    },
    limits: { timeoutMs: 1000, maxRenderDepth: 8, maxRenderNodes: 250 },
    ...overrides
  };
}

function createTestModuleOverrides() {
  return {
    '@1flowbase/block-renderer/antd-facade': {
      Text(input: { children?: unknown; props?: { children?: unknown } }) {
        return {
          primitive: 'Text',
          props: { children: input.props?.children ?? input.children }
        };
      }
    }
  };
}

function findEffectMessage(
  messages: JsBlockWorkerToHostMessage[]
): Extract<JsBlockWorkerToHostMessage, { type: 'interface' }> {
  const message = messages.find((item) => item.type === 'interface');
  expect(message).toMatchObject({
    direction: 'worker_to_host',
    type: 'interface',
    requestId: 'restricted-block:block-1:code-1',
    effectId: expect.any(String)
  });
  return message as Extract<JsBlockWorkerToHostMessage, { type: 'interface' }>;
}

function withoutPhaseMessages(messages: JsBlockWorkerToHostMessage[]) {
  return messages.filter((message) => message.type !== 'phase');
}

describe('FrontStage restricted block worker runtime', () => {
  test('attaches the default JS block runtime to a worker-like scope', async () => {
    const scope = new FakeWorkerScope();
    const attached = attachFrontstageRestrictedBlockWorkerRuntime(scope, {
      moduleOverrides: createTestModuleOverrides()
    });

    scope.emit({ direction: 'host_to_worker', type: 'init' });
    await attached.flush();

    scope.emit({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });
    const effectMessage = findEffectMessage(scope.messages);

    scope.emit({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'restricted-block:block-1:code-1',
      effectId: effectMessage.effectId,
      ok: true,
      value: { title: 'Ready' }
    });
    await attached.flush();

    scope.emit({ direction: 'host_to_worker', type: 'dispose' });
    await attached.flush();
    scope.emit({ direction: 'host_to_worker', type: 'init' });
    await attached.flush();
    attached.dispose();

    expect(withoutPhaseMessages(scope.messages)).toEqual([
      { direction: 'worker_to_host', type: 'ready' },
      {
        direction: 'worker_to_host',
        type: 'interface',
        requestId: 'restricted-block:block-1:code-1',
        method: 'GET',
        path: '/api/console/test',
        request: { query: { limit: 1 } },
        effectId: effectMessage.effectId
      },
      {
        direction: 'worker_to_host',
        type: 'event',
        requestId: 'restricted-block:block-1:code-1',
        name: 'record.loaded',
        payload: { title: 'Ready' }
      },
      {
        direction: 'worker_to_host',
        type: 'completed',
        requestId: 'restricted-block:block-1:code-1',
        view: { primitive: 'Text', props: { children: 'Ready' } },
        outputs: { title: 'Ready' }
      }
    ]);
  });
});

describe('FrontStage restricted block worker factory', () => {
  test('keeps only one pristine worker warm and releases it after TTL', () => {
    FakeNativeWorker.instances.length = 0;
    vi.useFakeTimers();
    vi.stubGlobal('Worker', FakeNativeWorker);

    try {
      const factory = createFrontstageRestrictedBlockWorkerFactory();
      factory();
      vi.advanceTimersByTime(0);

      expect(FakeNativeWorker.instances).toHaveLength(2);
      expect(FakeNativeWorker.instances[1]?.terminateCount).toBe(0);

      vi.advanceTimersByTime(30_000);
      expect(FakeNativeWorker.instances[1]?.terminateCount).toBe(1);
    } finally {
      vi.useRealTimers();
      vi.unstubAllGlobals();
    }
  });

  test('uses the app worker URL and module Worker options by default', () => {
    FakeNativeWorker.instances.length = 0;

    const factory = createFrontstageRestrictedBlockWorkerFactory({
      workerConstructor: FakeNativeWorker
    });

    factory();

    expect(FakeNativeWorker.instances).toHaveLength(1);
    expect(String(FakeNativeWorker.instances[0]?.scriptUrl)).toBe(
      String(getFrontstageRestrictedBlockWorkerUrl())
    );
    expect(String(FakeNativeWorker.instances[0]?.scriptUrl)).toContain(
      '/features/frontstage/workers/restricted-block-runtime.worker.ts'
    );
    expect(FakeNativeWorker.instances[0]?.options).toEqual({
      type: 'module',
      name: 'frontstage-restricted-block-runtime'
    });
    expect(getFrontstageRestrictedBlockWorkerOptions()).toEqual({
      type: 'module',
      name: 'frontstage-restricted-block-runtime'
    });
    expect(getFrontstageRestrictedBlockRuntimeFingerprint()).toBe(
      createCompiledBlockRuntimeFingerprint(
        getFrontstageRestrictedBlockWorkerUrl()
      )
    );
  });

  test('supports injected Worker constructors and URLs', () => {
    FakeNativeWorker.instances.length = 0;
    const workerUrl = new URL('https://example.test/custom-worker.js');

    const factory = createFrontstageRestrictedBlockWorkerFactory({
      workerConstructor: FakeNativeWorker,
      workerUrl
    });

    factory();

    expect(FakeNativeWorker.instances).toHaveLength(1);
    expect(FakeNativeWorker.instances[0]?.scriptUrl).toBe(workerUrl);
    expect(FakeNativeWorker.instances[0]?.options).toEqual({
      type: 'module',
      name: 'frontstage-restricted-block-runtime'
    });
  });
});

describe('FrontStage restricted block source policy', () => {
  test('keeps browser escape APIs denied and does not expand default imports', () => {
    expect(JS_BLOCK_ALLOWED_IMPORTS).toEqual([
      '@1flowbase/block-sdk',
      '@1flowbase/block-renderer/antd-facade'
    ]);

    expect(validateJsBlockSource('window.location.href;')).toMatchObject({
      ok: false,
      errors: [{ path: 'source.identifiers.window' }]
    });
    expect(validateJsBlockSource("await fetch('/api/private');")).toMatchObject(
      {
        ok: false,
        errors: [{ path: 'source.identifiers.fetch' }]
      }
    );
    expect(
      validateJsBlockSource(
        "import { z } from '@1flowbase/not-open';\nexport default z;"
      )
    ).toMatchObject({
      ok: false,
      errors: [{ path: 'source.imports[0]' }]
    });
    expect(
      validateJsBlockSource(
        "import { Text } from '@1flowbase/antd-facade';\nexport default Text;"
      )
    ).toMatchObject({
      ok: false,
      errors: [{ path: 'source.imports[0]' }]
    });
  });
});
