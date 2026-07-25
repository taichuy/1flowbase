import {
  createCompiledBlockRuntimeFingerprint,
  createJsBlockBrowserWorkerFactory,
  type JsBlockWorkerFactory
} from '@1flowbase/page-runtime';

import defaultJsBlockWorkerUrl from './default-js-block-runtime.worker?worker&url';

export function createDefaultJsBlockWorkerFactory(): JsBlockWorkerFactory {
  return createJsBlockBrowserWorkerFactory({
    workerUrl: defaultJsBlockWorkerUrl,
    workerOptions: { type: 'module', name: 'default-js-block-runtime' }
  });
}

export function getDefaultJsBlockRuntimeFingerprint(): string {
  return createCompiledBlockRuntimeFingerprint(defaultJsBlockWorkerUrl);
}
