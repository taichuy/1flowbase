export const BLOCK_RUNTIME_ERROR_CODES = [
  'import_denied',
  'syntax_invalid',
  'transform_failed',
  'runtime_timeout',
  'runtime_error',
  'schema_invalid',
  'query_denied',
  'create_denied',
  'update_denied',
  'delete_denied',
  'action_denied',
  'interface_denied',
  'event_denied'
] as const;

export type BlockRuntimeErrorCode = (typeof BLOCK_RUNTIME_ERROR_CODES)[number];

export interface BlockProtocolError {
  code: BlockRuntimeErrorCode;
  path: string;
  message: string;
  sourceLocation?: BlockSourceLocation;
}

export type BlockRuntimeDiagnosticPhase =
  | 'compile'
  | 'runtime'
  | 'data'
  | 'action'
  | 'interface'
  | 'event';

export interface BlockSourceLocation {
  line: number;
  column: number;
  endLine?: number;
  endColumn?: number;
}

export interface BlockRuntimeDiagnostic {
  pageId: string;
  tabId: string;
  blockId: string;
  phase: BlockRuntimeDiagnosticPhase;
  code: BlockRuntimeErrorCode;
  message: string;
  sourceLocation?: BlockSourceLocation;
}

export function createBlockRuntimeDiagnostic(
  diagnostic: BlockRuntimeDiagnostic
): BlockRuntimeDiagnostic {
  return diagnostic;
}
