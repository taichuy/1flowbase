import {
  attachDefaultJsBlockWorkerRuntime,
  type JsBlockWorkerRuntimeScope
} from '@1flowbase/page-runtime';

attachDefaultJsBlockWorkerRuntime(
  self as unknown as JsBlockWorkerRuntimeScope
);
