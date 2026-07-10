import * as antdFacade from '@1flowbase/block-renderer/antd-facade';
import * as blockSdk from '@1flowbase/block-sdk';

import {
  attachJsBlockWorkerRuntime,
  createJsBlockWorkerExecutor,
  type AttachedJsBlockWorkerRuntime,
  type JsBlockWorkerExecutor,
  type JsBlockWorkerExecutorOptions,
  type JsBlockWorkerRuntimeScope
} from './js-block-worker-executor';
import type { JsBlockInjectedModuleMap } from './js-block-source-evaluator';
import { JS_BLOCK_ALLOWED_IMPORTS } from './js-block-source-policy';
import {
  createFrontendBlockCodeCapabilities,
  type FrontendBlockCodeContribution
} from '@1flowbase/page-protocol';

export const JS_BLOCK_DEFAULT_MODULE_SOURCES = JS_BLOCK_ALLOWED_IMPORTS;

export interface DefaultJsBlockWorkerModulesOptions {
  moduleOverrides?: JsBlockInjectedModuleMap;
}

export type DefaultJsBlockWorkerExecutorOptions = Omit<
  JsBlockWorkerExecutorOptions,
  'modules'
> &
  DefaultJsBlockWorkerModulesOptions;

export type DefaultAttachedJsBlockWorkerRuntimeOptions =
  DefaultJsBlockWorkerModulesOptions;

export function createDefaultJsBlockInjectedModules(
  options: DefaultJsBlockWorkerModulesOptions = {}
): JsBlockInjectedModuleMap {
  return {
    '@1flowbase/block-sdk': blockSdk as Record<string, unknown>,
    '@1flowbase/block-renderer/antd-facade': antdFacade as Record<string, unknown>,
    ...(options.moduleOverrides ?? {})
  };
}

export function createCatalogJsBlockRuntimeCapabilities(
  contribution: FrontendBlockCodeContribution
) {
  const capabilities = createFrontendBlockCodeCapabilities(contribution);
  const defaultModules = createDefaultJsBlockInjectedModules();
  return {
    ...capabilities,
    workerModules: Object.fromEntries(
      capabilities.workerModuleSources.map((source) => [
        source,
        defaultModules[source]
      ])
    ) as JsBlockInjectedModuleMap
  };
}

export function createDefaultJsBlockWorkerExecutor(
  options: DefaultJsBlockWorkerExecutorOptions = {}
): JsBlockWorkerExecutor {
  return createJsBlockWorkerExecutor({
    postMessage: options.postMessage,
    modules: createDefaultJsBlockInjectedModules({
      moduleOverrides: options.moduleOverrides
    })
  });
}

export function attachDefaultJsBlockWorkerRuntime(
  scope: JsBlockWorkerRuntimeScope,
  options: DefaultAttachedJsBlockWorkerRuntimeOptions = {}
): AttachedJsBlockWorkerRuntime {
  return attachJsBlockWorkerRuntime(scope, {
    modules: createDefaultJsBlockInjectedModules({
      moduleOverrides: options.moduleOverrides
    })
  });
}
