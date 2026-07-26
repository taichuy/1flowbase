import {
  attachNativeReactCompilerWorker,
  type NativeReactCompilerWorkerScope
} from '@1flowbase/page-runtime';

attachNativeReactCompilerWorker(
  self as unknown as NativeReactCompilerWorkerScope
);
