import resolvedDependencyDeclarations from 'virtual:1flowbase-native-module-declarations';

import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

import { createFrontendModuleExtraLib } from './declarations';
import { FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS } from './registry';
import { FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES } from './resolved-dependency-sources';

const resolvedDependencySources: ReadonlySet<string> = new Set(
  FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES
);

export const FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS: readonly BlockSourceExtraLib[] =
  [
    ...resolvedDependencyDeclarations,
    ...FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS.filter(
      ({ module_source }) => !resolvedDependencySources.has(module_source)
    ).map(({ module_source, exports }) =>
      createFrontendModuleExtraLib(module_source, exports)
    )
  ];
