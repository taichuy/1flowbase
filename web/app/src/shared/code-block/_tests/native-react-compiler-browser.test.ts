import { describe, expect, test, vi } from 'vitest';

import { handleNativeReactCompilerRequest } from '@1flowbase/page-runtime';

import {
  NATIVE_REACT_COMPILER_WORKER_NAME,
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
