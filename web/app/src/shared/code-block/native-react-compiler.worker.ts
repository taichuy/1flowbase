import {
  attachNativeReactCompilerWorker,
  type NativeReactCompilerWorkerScope
} from '@1flowbase/page-runtime/compiler-worker';

attachNativeReactCompilerWorker(
  self as unknown as NativeReactCompilerWorkerScope
);
