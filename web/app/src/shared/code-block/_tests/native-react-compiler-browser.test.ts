import { describe, expect, test, vi } from 'vitest';

import { handleNativeReactCompilerRequest } from '@1flowbase/page-runtime';

import {
  NATIVE_REACT_COMPILER_WORKER_NAME,
  NativeReactBrowserCompilerPool,
  compileNativeReactComponentInBrowser,
  createNativeReactBrowserCompilerWorkerFactory,
  getNativeReactCompilerWorkerUrl,
  type NativeReactBrowserCompilerWorker
} from '../native-react-compiler-browser';

class FakeBrowserWorker implements NativeReactBrowserCompilerWorker {
  static instances: FakeBrowserWorker[] = [];
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  readonly terminate = vi.fn();

  constructor(
    readonly scriptUrl: string | URL,
    readonly options?: WorkerOptions
  ) {
    FakeBrowserWorker.instances.push(this);
  }

  postMessage(
    message: Parameters<NativeReactBrowserCompilerWorker['postMessage']>[0]
  ) {
    const response = handleNativeReactCompilerRequest(message);
    queueMicrotask(() => this.onmessage?.({ data: response } as MessageEvent));
  }
}

describe('Native React browser compiler adapter', () => {
  test('I2012-AC-002 aborts a stalled Worker and ignores its late result', async () => {
    const controller = new AbortController();
    const worker: NativeReactBrowserCompilerWorker = {
      onmessage: null,
      onerror: null,
      postMessage: vi.fn(),
      terminate: vi.fn()
    };
    const compilation = compileNativeReactComponentInBrowser({
      requestId: 'cancelled',
      source: 'export default () => null;',
      moduleDefinitions: [],
      workerFactory: () => worker,
      signal: controller.signal
    });
    controller.abort();
    expect(await compilation).toMatchObject({ ok: false });
    expect(worker.terminate).toHaveBeenCalledOnce();
    expect(worker.onmessage).toBeNull();
    const factory = vi.fn(() => worker);
    expect(
      await compileNativeReactComponentInBrowser({
        requestId: 'already-cancelled',
        source: '',
        moduleDefinitions: [],
        workerFactory: factory,
        signal: controller.signal
      })
    ).toMatchObject({ ok: false });
    expect(factory).not.toHaveBeenCalled();
  });
  test('D1-AC-001 uses the real bundled Worker URL and module Worker contract', async () => {
    FakeBrowserWorker.instances = [];
    const workerFactory = createNativeReactBrowserCompilerWorkerFactory({
      workerConstructor: FakeBrowserWorker
    });
    const result = await compileNativeReactComponentInBrowser({
      requestId: 'browser-compile-1',
      source: 'export default function Block() { return <div>Ready</div>; }',
      moduleDefinitions: [
        {
          module_source: 'react/jsx-runtime',
          exports: ['Fragment', 'jsx', 'jsxs']
        }
      ],
      workerFactory
    });

    expect(FakeBrowserWorker.instances[0]).toMatchObject({
      scriptUrl: getNativeReactCompilerWorkerUrl(),
      options: {
        type: 'module',
        name: NATIVE_REACT_COMPILER_WORKER_NAME
      }
    });
    if (!result.ok) throw new Error(JSON.stringify(result.diagnostics));
    expect(FakeBrowserWorker.instances[0]?.terminate).toHaveBeenCalledTimes(1);
  });

  test('I1967-AC-001 preserves raw TypeScript source identity across the Worker boundary', async () => {
    const source = `const tokenize = (input: string): string[] => {
      const tokens: string[] = [];
      const regex = /"([^"]*)"|([^,\\n]+)/g;
      return tokens.concat(regex.test(input) ? input : []);
    };
    export default () => <div>{tokenize('"value"').join(',')}</div>;`;

    const result = await compileNativeReactComponentInBrowser({
      requestId: 'issue-1967-browser-worker-source-identity',
      source,
      moduleDefinitions: [
        {
          module_source: 'react/jsx-runtime',
          exports: ['Fragment', 'jsx', 'jsxs']
        }
      ],
      workerFactory: () => new FakeBrowserWorker('test-worker')
    });

    expect(result).toMatchObject({ ok: true, diagnostics: [] });
  });
});

describe('reusable compiler pool', () => {
  const input = {
    source: 'export default () => null;',
    requestId: 'block',
    moduleDefinitions: []
  };
  test('reuses the Worker while every debug request runs the compiler', async () => {
    const worker = new FakeBrowserWorker('warm');
    const post = vi.spyOn(worker, 'postMessage');
    const pool = new NativeReactBrowserCompilerPool(() => worker, 1);
    try {
      for (let i = 0; i < 5; i++)
        expect(await pool.compile(input)).toMatchObject({ ok: true });
      expect(post).toHaveBeenCalledTimes(5);
      expect(
        new Set(post.mock.calls.map(([request]) => request.requestId)).size
      ).toBe(5);
      expect(worker.terminate).not.toHaveBeenCalled();
    } finally {
      pool.dispose();
    }
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  test('bounds active Workers, shares ordinary tasks, and isolates consumer abort/late replies', async () => {
    const workers: NativeReactBrowserCompilerWorker[] = [];
    const requests: Array<
      Parameters<NativeReactBrowserCompilerWorker['postMessage']>[0]
    > = [];
    const pool = new NativeReactBrowserCompilerPool(() => {
      const worker = {
        onmessage: null,
        onerror: null,
        terminate: vi.fn(),
        postMessage: vi.fn((request) => requests.push(request))
      } as NativeReactBrowserCompilerWorker;
      workers.push(worker);
      return worker;
    }, 1);
    const controller = new AbortController();
    const first = pool.compile({
      ...input,
      flightKey: 'tenant/source/policy',
      signal: controller.signal
    });
    const shared = pool.compile({
      ...input,
      flightKey: 'tenant/source/policy'
    });
    const debug = pool.compile(input);
    expect(workers).toHaveLength(1);
    expect(requests).toHaveLength(1);
    controller.abort();
    expect(await first).toMatchObject({ ok: false });
    expect(workers[0].terminate).not.toHaveBeenCalled();
    workers[0].onmessage?.({
      data: handleNativeReactCompilerRequest(requests[0])
    } as MessageEvent);
    expect(await shared).toMatchObject({ ok: true });
    expect(requests).toHaveLength(2);
    workers[0].onmessage?.({
      data: handleNativeReactCompilerRequest(requests[1])
    } as MessageEvent);
    expect(await debug).toMatchObject({ ok: true });

    const abandoned = new AbortController();
    const late = pool.compile({ ...input, signal: abandoned.signal });
    const oldHandler = workers[0].onmessage;
    const oldRequest = requests[2];
    abandoned.abort();
    expect(await late).toMatchObject({ ok: false });
    const next = pool.compile(input);
    expect(workers).toHaveLength(2);
    oldHandler?.({
      data: handleNativeReactCompilerRequest(oldRequest)
    } as MessageEvent);
    workers[1].onmessage?.({
      data: handleNativeReactCompilerRequest(requests[3])
    } as MessageEvent);
    expect(await next).toMatchObject({ ok: true });
    pool.dispose();
  });

  test('disposing running and queued jobs settles all consumers and terminates each Worker once', async () => {
    const worker: NativeReactBrowserCompilerWorker = {
      onmessage: null,
      onerror: null,
      postMessage: vi.fn(),
      terminate: vi.fn()
    };
    const pool = new NativeReactBrowserCompilerPool(() => worker, 1);
    const running = pool.compile(input),
      queued = pool.compile(input);
    pool.dispose();
    pool.dispose();
    expect(await running).toMatchObject({ ok: false });
    expect(await queued).toMatchObject({ ok: false });
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  test('retires a crashed or invalid-protocol Worker and compiles again on a fresh Worker', async () => {
    const workers: NativeReactBrowserCompilerWorker[] = [];
    const pool = new NativeReactBrowserCompilerPool(() => {
      const worker = new FakeBrowserWorker('recover');
      worker.postMessage = vi.fn();
      workers.push(worker);
      return worker;
    }, 1);
    const crashed = pool.compile(input);
    workers[0].onerror?.({ message: 'worker crash' } as ErrorEvent);
    expect(await crashed).toMatchObject({ ok: false });
    const invalid = pool.compile(input);
    workers[1].onmessage?.({
      data: {
        direction: 'worker_to_host',
        type: 'native_react_component_compiled',
        requestId: 'wrong'
      }
    } as MessageEvent);
    expect(await invalid).toMatchObject({ ok: false });
    expect(workers[0].terminate).toHaveBeenCalledOnce();
    expect(workers[1].terminate).toHaveBeenCalledOnce();
    pool.dispose();
  });

  test('defaults to maxWorkers = 1 in DEV so concurrent unbundled blocks do not flood dev tunnels', async () => {
    const workers: FakeBrowserWorker[] = [];
    const pool = new NativeReactBrowserCompilerPool(() => {
      const worker = new FakeBrowserWorker('singleton');
      workers.push(worker);
      return worker;
    });
    try {
      const results = await Promise.all([
        pool.compile({ ...input, flightKey: 'block-1' }),
        pool.compile({ ...input, flightKey: 'block-2' }),
        pool.compile({ ...input, flightKey: 'block-3' })
      ]);
      expect(results).toEqual([
        expect.objectContaining({ ok: true }),
        expect.objectContaining({ ok: true }),
        expect.objectContaining({ ok: true })
      ]);
      expect(workers).toHaveLength(1);
    } finally {
      pool.dispose();
    }
  });

  test('gates cold start to 1 lead Worker then bursts up to hardware maxWorkers in parallel once script cache is warm', async () => {
    const workers: Array<{
      worker: FakeBrowserWorker;
      pendingRequest: unknown;
      deliver: () => void;
    }> = [];
    const pool = new NativeReactBrowserCompilerPool(
      () => {
        const worker = new FakeBrowserWorker(`parallel-${workers.length}`);
        const originalPostMessage = worker.postMessage.bind(worker);
        const slot = {
          worker,
          pendingRequest: null as unknown,
          deliver: () => {}
        };
        worker.postMessage = vi.fn((message) => {
          slot.pendingRequest = message;
          slot.deliver = () => originalPostMessage(message);
        });
        workers.push(slot);
        return worker;
      },
      4,
      { cacheGateInitialWorker: true }
    );
    try {
      const flight1 = pool.compile({ ...input, flightKey: 'cold-1' });
      const flight2 = pool.compile({ ...input, flightKey: 'cold-2' });
      const flight3 = pool.compile({ ...input, flightKey: 'cold-3' });
      const flight4 = pool.compile({ ...input, flightKey: 'cold-4' });

      // Cold cache: strictly 1 lead Worker spawned while script cache is cold
      expect(workers).toHaveLength(1);

      // Once script cache warms (either via fetch warmup or first Worker response),
      // remaining queued blocks burst across parallel CPU Workers simultaneously
      pool.markScriptCacheWarm();
      expect(workers).toHaveLength(4);

      for (const slot of workers) {
        slot.deliver();
      }
      const settled = await Promise.all([flight1, flight2, flight3, flight4]);
      expect(settled).toEqual([
        expect.objectContaining({ ok: true }),
        expect.objectContaining({ ok: true }),
        expect.objectContaining({ ok: true }),
        expect.objectContaining({ ok: true })
      ]);
    } finally {
      pool.dispose();
    }
  });

  test('reclaims idle burst subordinate Workers after TTL while keeping 1 resident warm Worker', async () => {
    vi.useFakeTimers();
    const workers: FakeBrowserWorker[] = [];
    const pool = new NativeReactBrowserCompilerPool(
      () => {
        const worker = new FakeBrowserWorker(`reclaim-${workers.length}`);
        workers.push(worker);
        return worker;
      },
      4,
      { cacheGateInitialWorker: false, idleWorkerTtlMs: 5_000 }
    );
    try {
      const settled = await Promise.all([
        pool.compile({ ...input, flightKey: 'burst-1' }),
        pool.compile({ ...input, flightKey: 'burst-2' }),
        pool.compile({ ...input, flightKey: 'burst-3' }),
        pool.compile({ ...input, flightKey: 'burst-4' })
      ]);
      expect(settled).toHaveLength(4);
      expect(workers).toHaveLength(4);
      expect(
        workers.filter((w) => w.terminate.mock.calls.length > 0)
      ).toHaveLength(0);

      // Before TTL elapses, subordinate Workers remain available for follow-up bursts
      vi.advanceTimersByTime(4_900);
      expect(
        workers.filter((w) => w.terminate.mock.calls.length > 0)
      ).toHaveLength(0);

      // Once idle TTL elapses, 3 subordinate Workers terminate and 1 warm Worker stays resident
      vi.advanceTimersByTime(200);
      expect(
        workers.filter((w) => w.terminate.mock.calls.length > 0)
      ).toHaveLength(3);

      // Subsequent single-block compile reuses the 1 surviving warm Worker without spawning a 5th
      const followUp = await pool.compile({ ...input, flightKey: 'warm-edit-1' });
      expect(followUp).toMatchObject({ ok: true });
      expect(workers).toHaveLength(4);
    } finally {
      pool.dispose();
      vi.useRealTimers();
    }
  });

  test('preempts queued P2 near-viewport preloads with P0/P1 visible viewport blocks and supports scroll promotion', async () => {
    const dispatchedOrder: string[] = [];
    let releaseLeadWorker!: () => void;
    const pool = new NativeReactBrowserCompilerPool(
      () => {
        const worker = new FakeBrowserWorker('prio-worker');
        const originalPost = worker.postMessage.bind(worker);
        let firstCall = true;
        worker.postMessage = vi.fn(
          (message: Parameters<FakeBrowserWorker['postMessage']>[0]) => {
            dispatchedOrder.push(message.requestId.split(':worker:')[0]!);
            if (firstCall) {
              firstCall = false;
              releaseLeadWorker = () => originalPost(message);
              return;
            }
            originalPost(message);
          }
        );
        return worker;
      },
      1,
      { cacheGateInitialWorker: false }
    );
    try {
      const lead = pool.compile({
        ...input,
        requestId: 'slot-0-visible',
        flightKey: 'f-0',
        priority: 1,
        slotIndex: 0
      });
      const nearA = pool.compile({
        ...input,
        requestId: 'slot-2-preload',
        flightKey: 'f-2',
        priority: 2,
        slotIndex: 2
      });
      const nearB = pool.compile({
        ...input,
        requestId: 'slot-3-preload',
        flightKey: 'f-3',
        priority: 2,
        slotIndex: 3
      });
      // User scrolls down: a newly visible block (priority=1) arrives AND slot-3 is promoted to selected (priority=0)!
      const newlyVisible = pool.compile({
        ...input,
        requestId: 'slot-5-scrolled-visible',
        flightKey: 'f-5',
        priority: 1,
        slotIndex: 5
      });
      pool.promote('f-3', 0);

      releaseLeadWorker();
      await Promise.all([lead, nearA, nearB, newlyVisible]);

      // Expected dequeue order after lead: promoted P0 (slot-3) -> visible P1 (slot-5) -> preload P2 (slot-2)
      expect(dispatchedOrder).toEqual([
        'slot-0-visible',
        'slot-3-preload',
        'slot-5-scrolled-visible',
        'slot-2-preload'
      ]);
    } finally {
      pool.dispose();
    }
  });
});
