import * as antdModule from 'antd';
import * as ReactModule from 'react';
import * as ReactJsxRuntimeModule from 'react/jsx-runtime';
import * as uiModule from '@1flowbase/ui';

import {
  evaluateNativeTrustedBlockSource,
  createNativeReactModuleRegistry,
  nativeReactCatalogDependencyLockIdentity,
  type NativeTrustedBlockRunError,
  type NativeReactCatalogDependencyLock,
  type NativeReactModuleRegistry,
  type NativeTrustedBlockInjectedModuleMap,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';

import type { FrontstageNativeTrustedBlockReactComponent } from './native-trusted-block-react-adapter';

export {
  FRONTSTAGE_NATIVE_TRUSTED_BLOCK_COMPATIBILITY_CONTRACT_VERSION,
  getFrontstageNativeTrustedBlockRuntimeCompatibility,
  type FrontstageNativeTrustedBlockRuntimeCompatibilityManifest,
  type FrontstageNativeTrustedBlockRuntimeCompatibilityModule
} from './native-trusted-block-runtime-compatibility';

type InjectedModule = Record<string, unknown>;
const sharedNativeReactModuleRegistries = new Map<
  string,
  NativeReactModuleRegistry
>();

export interface FrontstageNativeTrustedBlockRuntimeFactoryOptions {
  modules?: NativeTrustedBlockInjectedModuleMap;
}

export class FrontstageNativeTrustedBlockRuntimeError extends Error {
  readonly kind: NativeTrustedBlockRunError['kind'];
  readonly errors: NativeTrustedBlockRunError['errors'];

  constructor(error: NativeTrustedBlockRunError) {
    super(error.message);
    this.name = 'FrontstageNativeTrustedBlockRuntimeError';
    this.kind = error.kind;
    this.errors = error.errors;
  }
}

export function createFrontstageNativeTrustedBlockRuntimeFactory(
  options: FrontstageNativeTrustedBlockRuntimeFactoryOptions = {}
): (
  plan: NativeTrustedBlockPreparePlan
) => FrontstageNativeTrustedBlockReactComponent {
  const modules = createFrontstageNativeTrustedBlockModuleMap(options.modules);

  return (plan) => {
    const result = evaluateNativeTrustedBlockSource({
      source: plan.source,
      modules
    });

    if (!result.ok) {
      throw new FrontstageNativeTrustedBlockRuntimeError(result.error);
    }

    return result.component as FrontstageNativeTrustedBlockReactComponent;
  };
}

export function createFrontstageNativeTrustedBlockModuleMap(
  overrides: NativeTrustedBlockInjectedModuleMap = {}
): NativeTrustedBlockInjectedModuleMap {
  return {
    react: mergeInjectedModule(createReactModule(), overrides.react),
    'react/jsx-runtime': mergeInjectedModule(
      ReactJsxRuntimeModule,
      overrides['react/jsx-runtime']
    ),
    antd: mergeInjectedModule(antdModule, overrides.antd),
    '@1flowbase/ui': mergeInjectedModule(uiModule, overrides['@1flowbase/ui'])
  };
}

export function createFrontstageNativeReactModuleRegistry(
  dependencyLock: NativeReactCatalogDependencyLock,
  options: { fetchAsset?: typeof fetch } = {}
): NativeReactModuleRegistry {
  const sharedKey = options.fetchAsset
    ? null
    : nativeReactCatalogDependencyLockIdentity(dependencyLock);
  const shared = sharedKey
    ? sharedNativeReactModuleRegistries.get(sharedKey)
    : undefined;
  if (shared) return shared;
  const registry = createNativeReactModuleRegistry({
    dependencyLock,
    hostModules: createFrontstageNativeTrustedBlockModuleMap(),
    ...(options.fetchAsset ? { fetchAsset: options.fetchAsset } : {})
  });
  if (sharedKey) sharedNativeReactModuleRegistries.set(sharedKey, registry);
  return registry;
}

function createReactModule(): InjectedModule {
  return {
    ...ReactModule,
    default: getReactDefaultExport()
  };
}

function getReactDefaultExport(): unknown {
  return 'default' in ReactModule ? ReactModule.default : ReactModule;
}

function mergeInjectedModule(
  defaults: InjectedModule,
  override: InjectedModule | undefined
): InjectedModule {
  return {
    ...defaults,
    ...(override ?? {})
  };
}
