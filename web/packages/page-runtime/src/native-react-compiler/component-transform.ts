import type { BlockProtocolError } from '@1flowbase/page-protocol';

import { validateNativeTrustedBlockSource } from '../native-trusted-block-source-policy';
import { RUNTIME_CAPABILITY_GUARD_BINDING_NAMES } from '../native-trusted-block/runtime-capability-guard';
import {
  DEFAULT_EXPORT_IDENTIFIER,
  MODULES_IDENTIFIER,
  RESERVED_TRANSFORM_IDENTIFIERS,
  applyEdits,
  collectInjectedModules,
  createModuleBindingPreamble,
  parseTopLevelModuleSyntax,
  tokenizeSource
} from '../native-trusted-block/source-evaluator-transform';
import type {
  NativeTrustedBlockImportBinding,
  NativeTrustedBlockInjectedModule
} from '../native-trusted-block/source-evaluator-types';
import { transformNativeReactTsx } from './tsx-transform';

export type NativeReactComponentTransformResult =
  | {
      ok: true;
      source: string;
      injectedModules: NativeTrustedBlockInjectedModule[];
      importBindings: NativeTrustedBlockImportBinding[];
      executableBody: string;
      executablePreambleLines: number;
      moduleMapIdentifier: string;
      runtimeCapabilityGuardBindingIdentifiers: readonly string[];
      defaultExportIdentifier: string;
      sourceMap: unknown;
      errors: [];
    }
  | { ok: false; errors: BlockProtocolError[] };

export function transformNativeReactComponentSource(
  source: unknown
): NativeReactComponentTransformResult {
  const policy = validateNativeTrustedBlockSource(source);
  if (!policy.ok) return policy;

  const reservedToken = tokenizeSource(policy.source).find((token) =>
    RESERVED_TRANSFORM_IDENTIFIERS.has(token.value)
  );
  if (reservedToken) {
    return transformFailure(
      'source.identifiers',
      `Identifier '${reservedToken.value}' is reserved by the Native React compiler.`
    );
  }

  const tsx = transformNativeReactTsx(policy.source);
  if (!tsx.ok) return tsx;

  const parsed = parseTopLevelModuleSyntax(tsx.code, tokenizeSource(tsx.code));
  if (!parsed.ok) return { ok: false, errors: [parsed.error] };
  const bindings = collectInjectedModules(parsed.value.imports);
  if (!bindings.ok) return { ok: false, errors: [bindings.error] };

  const executableSource = applyEdits(tsx.code, [
    ...parsed.value.imports.map((declaration) => ({
      start: declaration.start,
      end: declaration.end,
      replacement: preserveLineBreaks(
        tsx.code.slice(declaration.start, declaration.end)
      )
    })),
    {
      start: parsed.value.defaultExport.start,
      end: parsed.value.defaultExport.end,
      replacement: parsed.value.defaultExport.replacement
    }
  ]);
  const modulePreamble = createModuleBindingPreamble(
    bindings.value.injectedModules
  );

  return {
    ok: true,
    source: policy.source,
    injectedModules: bindings.value.injectedModules,
    importBindings: bindings.value.importBindings,
    executableBody: [
      ...modulePreamble,
      executableSource.trim(),
      `return ${DEFAULT_EXPORT_IDENTIFIER};`
    ].join('\n'),
    executablePreambleLines: modulePreamble.length,
    moduleMapIdentifier: MODULES_IDENTIFIER,
    runtimeCapabilityGuardBindingIdentifiers:
      RUNTIME_CAPABILITY_GUARD_BINDING_NAMES,
    defaultExportIdentifier: DEFAULT_EXPORT_IDENTIFIER,
    sourceMap: tsx.sourceMap,
    errors: []
  };
}

function preserveLineBreaks(value: string): string {
  return value.replace(/[^\r\n]/g, ' ');
}

function transformFailure(
  path: string,
  message: string
): NativeReactComponentTransformResult {
  return {
    ok: false,
    errors: [{ code: 'transform_failed', path, message }]
  };
}
