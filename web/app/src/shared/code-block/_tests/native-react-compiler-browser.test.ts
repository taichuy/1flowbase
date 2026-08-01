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
  test('D1-AC-001 uses the real bundled Worker URL and module Worker contract', async () => {
    FakeBrowserWorker.instances = [];
    const workerFactory = createNativeReactBrowserCompilerWorkerFactory({
      workerConstructor: FakeBrowserWorker
    });
    const result = await compileNativeReactComponentInBrowser({
      requestId: 'browser-compile-1',
      source: 'export default function Block() { return <div>Ready</div>; }',
      workerFactory
    });

    expect(FakeBrowserWorker.instances[0]).toMatchObject({
      scriptUrl: getNativeReactCompilerWorkerUrl(),
      options: {
        type: 'module',
        name: NATIVE_REACT_COMPILER_WORKER_NAME
      }
    });
    expect(result.ok).toBe(true);
    expect(FakeBrowserWorker.instances[0]?.terminate).toHaveBeenCalledTimes(1);
  });
});
