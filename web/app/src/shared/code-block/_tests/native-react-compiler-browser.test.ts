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
});
