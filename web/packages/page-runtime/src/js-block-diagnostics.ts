import type {
  BlockProtocolError,
  BlockRuntimeDiagnostic,
  BlockRuntimeDiagnosticPhase
} from '@1flowbase/page-protocol';

export interface JsBlockDiagnosticContext {
  pageId: string;
  tabId: string;
  blockId: string;
}

export function createJsBlockDiagnostics(
  context: JsBlockDiagnosticContext,
  errors: BlockProtocolError[]
): BlockRuntimeDiagnostic[] {
  return errors.map((error) => ({
    ...context,
    phase: diagnosticPhase(error.code),
    code: error.code,
    message: error.message,
    sourceLocation: error.sourceLocation
  }));
}

function diagnosticPhase(
  code: BlockProtocolError['code']
): BlockRuntimeDiagnosticPhase {
  if (code === 'query_denied') {
    return 'data';
  }
  if (
    code === 'create_denied' ||
    code === 'update_denied' ||
    code === 'delete_denied' ||
    code === 'action_denied' ||
    code === 'event_denied'
  ) {
    return 'action';
  }
  if (
    code === 'import_denied' ||
    code === 'syntax_invalid' ||
    code === 'transform_failed'
  ) {
    return 'compile';
  }
  return 'runtime';
}
