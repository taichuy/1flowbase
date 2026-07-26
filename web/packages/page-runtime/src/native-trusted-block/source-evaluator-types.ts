import type { BlockProtocolError } from '@1flowbase/page-protocol';

import type { JsBlockRunError } from '../js-block-worker-runtime';
export const NATIVE_REACT_JSX_RUNTIME_IMPORT_SOURCE =
  'react/jsx-runtime' as const;

export type NativeTrustedBlockInjectedModuleSource = string;

export type NativeTrustedBlockInjectedModuleMap = Record<
  string,
  Record<string, unknown> | undefined
>;

export type NativeTrustedBlockComponent = (...args: unknown[]) => unknown;

export type NativeTrustedBlockImportBinding =
  | {
      kind: 'named';
      source: NativeTrustedBlockInjectedModuleSource;
      imported: string;
      local: string;
    }
  | {
      kind: 'default';
      source: NativeTrustedBlockInjectedModuleSource;
      local: string;
    }
  | {
      kind: 'namespace';
      source: NativeTrustedBlockInjectedModuleSource;
      local: string;
    };

export interface NativeTrustedBlockInjectedModule {
  source: NativeTrustedBlockInjectedModuleSource;
  bindings: NativeTrustedBlockImportBinding[];
}

export interface NativeTrustedBlockSourceTransformSuccess {
  ok: true;
  source: string;
  normalizedSource: string;
  injectedModules: NativeTrustedBlockInjectedModule[];
  importBindings: NativeTrustedBlockImportBinding[];
  executableBody: string;
  moduleMapIdentifier: string;
  runtimeCapabilityGuardBindingIdentifiers: readonly string[];
  defaultExportIdentifier: string;
  errors: [];
}

export interface NativeTrustedBlockSourceTransformFailure {
  ok: false;
  errorKind: JsBlockRunError['kind'];
  errors: BlockProtocolError[];
}

export type NativeTrustedBlockSourceTransformResult =
  | NativeTrustedBlockSourceTransformSuccess
  | NativeTrustedBlockSourceTransformFailure;

export type NativeTrustedBlockSourceEvaluationResult =
  | {
      ok: true;
      component: NativeTrustedBlockComponent;
      compiledSource: NativeTrustedBlockSourceTransformSuccess;
      errors: [];
    }
  | {
      ok: false;
      error: JsBlockRunError;
    };

export interface EvaluateNativeTrustedBlockSourceInput {
  source: string;
  modules: NativeTrustedBlockInjectedModuleMap;
}

export interface SourceToken {
  value: string;
  start: number;
  end: number;
  depth: number;
}

export interface ImportDeclaration {
  source: NativeTrustedBlockInjectedModuleSource;
  bindings: NativeTrustedBlockImportBinding[];
  start: number;
  end: number;
}

export interface DefaultExportDeclaration {
  start: number;
  end: number;
  replacement: string;
}

export interface SourceEdit {
  start: number;
  end: number;
  replacement: string;
}

export interface StringLiteralValue {
  value: string;
  end: number;
}

export interface StatementEnd {
  expressionEnd: number;
  statementEnd: number;
}

export interface ParseSuccess<T> {
  ok: true;
  value: T;
}

export interface ParseFailure {
  ok: false;
  error: BlockProtocolError;
}

export type ParseResult<T> = ParseSuccess<T> | ParseFailure;
