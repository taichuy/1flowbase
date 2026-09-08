import type { NativeTrustedBlockRunError } from './source-evaluator-types';

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
