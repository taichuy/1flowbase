import {
  validateBlockUiSchema,
  type BlockContext,
  type BlockProtocolError,
  type BlockUiSchemaValidationOptions
} from '@1flowbase/page-protocol';
import {
  isBlockModule,
  isBlockResult,
  type BlockModule,
  type BlockResult
} from '@1flowbase/block-sdk';

import {
  transformJsBlockSource,
  type JsBlockInjectedModuleSource,
  type JsBlockSourceTransformResult,
  type JsBlockSourceTransformSuccess
} from './js-block-source-transform';
import { compileJsBlockTsxSource } from './js-block-tsx-compile';
import type { JsBlockRunError } from './js-block-worker-runtime';

export type JsBlockInjectedModuleMap = Partial<
  Record<JsBlockInjectedModuleSource, Record<string, unknown>>
>;

export type JsBlockSourceEvaluationResult =
  | {
      ok: true;
      compiledSource: JsBlockSourceTransformSuccess;
      block: BlockModule;
    }
  | {
      ok: false;
      error: JsBlockRunError;
    };

export type JsBlockSourceRunResult =
  | {
      ok: true;
      compiledSource: JsBlockSourceTransformSuccess;
      result: BlockResult;
    }
  | {
      ok: false;
      error: JsBlockRunError;
    };

export interface EvaluateJsBlockSourceInput {
  source: string | JsBlockSourceTransformSuccess;
  modules: JsBlockInjectedModuleMap;
}

export interface RunJsBlockSourceInput extends EvaluateJsBlockSourceInput {
  context: BlockContext;
  validationOptions?: BlockUiSchemaValidationOptions;
}

export function evaluateJsBlockSource(
  input: EvaluateJsBlockSourceInput
): JsBlockSourceEvaluationResult {
  const compiledSource =
    typeof input.source === 'string'
      ? compileAndTransformSource(input.source, Object.keys(input.modules))
      : input.source;

  if (!compiledSource.ok) {
    return {
      ok: false,
      error: createRunError(
        'source_policy_failed',
        'JS block source transform failed.',
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
    const defaultExport = evaluator(input.modules);
    if (!isBlockModule(defaultExport)) {
      return {
        ok: false,
        error: runtimeError(
          'source.defaultExport',
          'JS block default export must be a BlockModule with main(ctx).'
        )
      };
    }

    return {
      ok: true,
      compiledSource,
      block: defaultExport
    };
  } catch (error) {
    return {
      ok: false,
      error: runtimeError(
        'runtime.evaluate',
        `JS block source evaluation failed: ${getErrorMessage(error)}`
      )
    };
  }
}

export async function runJsBlockSource(
  input: RunJsBlockSourceInput
): Promise<JsBlockSourceRunResult> {
  const evaluation = evaluateJsBlockSource(input);
  if (!evaluation.ok) {
    return evaluation;
  }

  let blockResult: unknown;
  try {
    blockResult = await evaluation.block.main(input.context);
  } catch (error) {
    return {
      ok: false,
      error: runtimeError(
        'runtime.main',
        `JS block main failed: ${getErrorMessage(error)}`
      )
    };
  }

  if (!isBlockResult(blockResult)) {
    return {
      ok: false,
      error: runtimeError(
        'runtime.result',
        'JS block main must return { view, outputs } with plain-object outputs.'
      )
    };
  }

  const validation = validateBlockUiSchema(
    blockResult.view,
    input.validationOptions
  );
  if (!validation.ok) {
    return {
      ok: false,
      error: createRunError(
        'schema_invalid',
        'BlockResult view validation failed.',
        validation.errors
      )
    };
  }

  return {
    ok: true,
    compiledSource: evaluation.compiledSource,
    result: {
      view: validation.schema,
      outputs: blockResult.outputs
    }
  };
}

function compileAndTransformSource(
  source: string,
  allowedImports: string[]
): JsBlockSourceTransformResult {
  const tsxResult = compileJsBlockTsxSource(source);
  if (!tsxResult.ok) {
    return { ok: false, errors: tsxResult.errors };
  }

  return transformJsBlockSource(tsxResult.code, { allowedImports });
}

function createEvaluator(
  compiledSource: JsBlockSourceTransformSuccess
): (modules: JsBlockInjectedModuleMap) => unknown {
  return new Function(
    compiledSource.moduleMapIdentifier,
    `"use strict";\n${compiledSource.executableBody}`
  ) as (modules: JsBlockInjectedModuleMap) => unknown;
}

function validateInjectedModules(
  compiledSource: JsBlockSourceTransformSuccess,
  modules: JsBlockInjectedModuleMap
): JsBlockRunError | null {
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

function createRunError(
  kind: JsBlockRunError['kind'],
  message: string,
  errors: BlockProtocolError[]
): JsBlockRunError {
  return { kind, message, errors };
}

function runtimeError(path: string, message: string): JsBlockRunError {
  return createRunError('runtime_error', message, [
    {
      code: 'runtime_error',
      path,
      message
    }
  ]);
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
