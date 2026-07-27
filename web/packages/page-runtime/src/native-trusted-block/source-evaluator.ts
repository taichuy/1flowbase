import type { BlockProtocolError } from '@1flowbase/page-protocol';

import { validateNativeTrustedBlockSource } from '../native-trusted-block-source-policy';
import { transformNativeTrustedBlockJsx } from './jsx-transform';
import {
  RUNTIME_CAPABILITY_GUARD_BINDING_NAMES,
  createNativeTrustedBlockRuntimeCapabilityGuardBindings,
  getNativeTrustedBlockRuntimeCapabilityGuardValues,
  isNativeTrustedBlockRuntimeCapabilityGuardError
} from './runtime-capability-guard';
import {
  DEFAULT_EXPORT_IDENTIFIER,
  MODULES_IDENTIFIER,
  RESERVED_TRANSFORM_IDENTIFIERS,
  applyEdits,
  collectInjectedModules,
  createModuleBindingPreamble,
  findReactJsxRuntimeIdentifier,
  parseTopLevelModuleSyntax,
  tokenizeSource
} from './source-evaluator-transform';
import type {
  EvaluateNativeTrustedBlockSourceInput,
  NativeTrustedBlockComponent,
  NativeTrustedBlockInjectedModuleMap,
  NativeTrustedBlockRunError,
  NativeTrustedBlockSourceEvaluationResult,
  NativeTrustedBlockSourceTransformFailure,
  NativeTrustedBlockSourceTransformResult,
  NativeTrustedBlockSourceTransformSuccess
} from './source-evaluator-types';

export type {
  EvaluateNativeTrustedBlockSourceInput,
  NativeTrustedBlockComponent,
  NativeTrustedBlockImportBinding,
  NativeTrustedBlockInjectedModule,
  NativeTrustedBlockInjectedModuleMap,
  NativeTrustedBlockInjectedModuleSource,
  NativeTrustedBlockRunError,
  NativeTrustedBlockSourceEvaluationResult,
  NativeTrustedBlockSourceTransformFailure,
  NativeTrustedBlockSourceTransformResult,
  NativeTrustedBlockSourceTransformSuccess
} from './source-evaluator-types';

export class NativeTrustedBlockRuntimeError extends Error {
  readonly kind: NativeTrustedBlockRunError['kind'];
  readonly errors: NativeTrustedBlockRunError['errors'];

  constructor(error: NativeTrustedBlockRunError) {
    super(error.message);
    this.name = 'NativeTrustedBlockRuntimeError';
    this.kind = error.kind;
    this.errors = error.errors;
  }
}

export function createNativeTrustedBlockRuntimeError(
  error: NativeTrustedBlockRunError
): NativeTrustedBlockRuntimeError {
  return new NativeTrustedBlockRuntimeError(error);
}

export function isNativeTrustedBlockRuntimeError(
  error: unknown
): error is NativeTrustedBlockRuntimeError {
  return (
    error instanceof NativeTrustedBlockRuntimeError ||
    (isRecord(error) &&
      error.name === 'NativeTrustedBlockRuntimeError' &&
      typeof error.message === 'string' &&
      isNativeTrustedBlockRunErrorKind(error.kind) &&
      Array.isArray(error.errors))
  );
}

export function evaluateNativeTrustedBlockSource(
  input: EvaluateNativeTrustedBlockSourceInput
): NativeTrustedBlockSourceEvaluationResult {
  const compiledSource = transformNativeTrustedBlockSource(input.source);
  if (!compiledSource.ok) {
    return {
      ok: false,
      error: createRunError(
        compiledSource.errorKind,
        compiledSource.errorKind === 'source_policy_failed'
          ? 'Native trusted block source policy failed.'
          : 'Native trusted block source transform failed.',
        compiledSource.errors
      )
    };
  }

  const moduleValidation = validateInjectedModules(
    compiledSource,
    input.modules
  );
  if (moduleValidation) {
    return { ok: false, error: moduleValidation };
  }

  try {
    const evaluator = createEvaluator(compiledSource);
    const runtimeCapabilityGuardBindings =
      createNativeTrustedBlockRuntimeCapabilityGuardBindings();
    const defaultExport = evaluator(
      input.modules,
      ...getNativeTrustedBlockRuntimeCapabilityGuardValues(
        runtimeCapabilityGuardBindings
      )
    );
    if (!isNativeTrustedBlockComponent(defaultExport)) {
      return {
        ok: false,
        error: runtimeError(
          'source.defaultExport',
          'Native trusted block default export must be a component function.'
        )
      };
    }

    return {
      ok: true,
      component: wrapNativeTrustedBlockComponent(defaultExport),
      compiledSource,
      errors: []
    };
  } catch (error) {
    if (isNativeTrustedBlockRuntimeCapabilityGuardError(error)) {
      return {
        ok: false,
        error: runtimeError(error.path, error.message)
      };
    }

    return {
      ok: false,
      error: runtimeError(
        'runtime.evaluate',
        `Native trusted block source evaluation failed: ${getErrorMessage(error)}`
      )
    };
  }
}

export function transformNativeTrustedBlockSource(
  source: unknown
): NativeTrustedBlockSourceTransformResult {
  const policyResult = validateNativeTrustedBlockSource(source);
  if (!policyResult.ok) {
    return {
      ok: false,
      errorKind: 'source_policy_failed',
      errors: policyResult.errors
    };
  }

  const tokens = tokenizeSource(policyResult.source);
  const reservedToken = tokens.find((token) =>
    RESERVED_TRANSFORM_IDENTIFIERS.has(token.value)
  );
  if (reservedToken) {
    return transformRuntimeFailed(
      'source.identifiers',
      `Identifier '${reservedToken.value}' is reserved by the native trusted block transform.`
    );
  }

  const parsed = parseTopLevelModuleSyntax(policyResult.source, tokens);
  if (!parsed.ok) {
    return {
      ok: false,
      errorKind: 'runtime_error',
      errors: [parsed.error]
    };
  }

  const { imports, defaultExport } = parsed.value;
  const bindingResult = collectInjectedModules(imports);
  if (!bindingResult.ok) {
    return {
      ok: false,
      errorKind: 'runtime_error',
      errors: [bindingResult.error]
    };
  }

  const executableSource = applyEdits(policyResult.source, [
    ...imports.map((importDeclaration) => ({
      start: importDeclaration.start,
      end: importDeclaration.end,
      replacement: ''
    })),
    {
      start: defaultExport.start,
      end: defaultExport.end,
      replacement: defaultExport.replacement
    }
  ]);
  const jsxResult = transformNativeTrustedBlockJsx(executableSource, {
    reactIdentifier: findReactJsxRuntimeIdentifier(
      bindingResult.value.importBindings
    ),
    componentIdentifiers: new Set(
      bindingResult.value.importBindings
        .filter((binding) => binding.source !== 'react')
        .map((binding) => binding.local)
    )
  });
  if (!jsxResult.ok) {
    return {
      ok: false,
      errorKind: 'runtime_error',
      errors: jsxResult.errors
    };
  }

  const executableBody = [
    ...createModuleBindingPreamble(bindingResult.value.injectedModules),
    jsxResult.source.trim(),
    `return ${DEFAULT_EXPORT_IDENTIFIER};`
  ]
    .filter((line) => line.length > 0)
    .join('\n');

  return {
    ok: true,
    source: policyResult.source,
    normalizedSource: policyResult.normalizedSource,
    injectedModules: bindingResult.value.injectedModules,
    importBindings: bindingResult.value.importBindings,
    executableBody,
    moduleMapIdentifier: MODULES_IDENTIFIER,
    runtimeCapabilityGuardBindingIdentifiers:
      RUNTIME_CAPABILITY_GUARD_BINDING_NAMES,
    defaultExportIdentifier: DEFAULT_EXPORT_IDENTIFIER,
    errors: []
  };
}

function createEvaluator(
  compiledSource: NativeTrustedBlockSourceTransformSuccess
): (
  modules: NativeTrustedBlockInjectedModuleMap,
  ...guardValues: unknown[]
) => unknown {
  return new Function(
    compiledSource.moduleMapIdentifier,
    ...compiledSource.runtimeCapabilityGuardBindingIdentifiers,
    `"use strict";\n${compiledSource.executableBody}`
  ) as (
    modules: NativeTrustedBlockInjectedModuleMap,
    ...guardValues: unknown[]
  ) => unknown;
}

function validateInjectedModules(
  compiledSource: NativeTrustedBlockSourceTransformSuccess,
  modules: NativeTrustedBlockInjectedModuleMap
): NativeTrustedBlockRunError | null {
  for (const injectedModule of compiledSource.injectedModules) {
    const moduleValue = modules[injectedModule.source];
    if (!isRecord(moduleValue)) {
      return runtimeError(
        `modules.${injectedModule.source}`,
        `Injected module is missing: ${injectedModule.source}.`
      );
    }

    for (const binding of injectedModule.bindings) {
      if (binding.kind === 'namespace') {
        continue;
      }

      const exportedName =
        binding.kind === 'default' ? 'default' : binding.imported;
      if (!(exportedName in moduleValue)) {
        return runtimeError(
          `modules.${injectedModule.source}.${exportedName}`,
          `Injected module binding is missing: ${injectedModule.source}.${exportedName}.`
        );
      }
    }
  }

  return null;
}

function isNativeTrustedBlockComponent(
  value: unknown
): value is NativeTrustedBlockComponent {
  return typeof value === 'function';
}

function wrapNativeTrustedBlockComponent(
  component: NativeTrustedBlockComponent
): NativeTrustedBlockComponent {
  return function guardedNativeTrustedBlockComponent(
    this: unknown,
    ...args: unknown[]
  ): unknown {
    try {
      return component.apply(this, args);
    } catch (error) {
      if (isNativeTrustedBlockRuntimeCapabilityGuardError(error)) {
        throw createNativeTrustedBlockRuntimeError(
          runtimeError(error.path, error.message)
        );
      }

      throw error;
    }
  };
}

function createRunError(
  kind: NativeTrustedBlockRunError['kind'],
  message: string,
  errors: BlockProtocolError[]
): NativeTrustedBlockRunError {
  return { kind, message, errors };
}

function runtimeError(
  path: string,
  message: string
): NativeTrustedBlockRunError {
  return createRunError('runtime_error', message, [
    {
      code: 'runtime_error',
      path,
      message
    }
  ]);
}

function transformRuntimeFailed(
  path: string,
  message: string
): NativeTrustedBlockSourceTransformFailure {
  return {
    ok: false,
    errorKind: 'runtime_error',
    errors: [
      {
        code: 'runtime_error',
        path,
        message
      }
    ]
  };
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  return 'unknown error';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNativeTrustedBlockRunErrorKind(
  value: unknown
): value is NativeTrustedBlockRunError['kind'] {
  return (
    value === 'runtime_error' ||
    value === 'source_policy_failed' ||
    value === 'schema_invalid' ||
    value === 'runtime_timeout'
  );
}
