import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';

import type { JsBlockInterfaceCallTrace } from './js-block-host-effect-bridge';
import type {
  JsBlockRunPhase,
  JsBlockRuntimeSessionState,
  JsBlockWorkerLogEntry
} from './js-block-worker-runtime';

export interface JsBlockDraftRun {
  run_id: string;
  draft_hash: string;
  context_snapshot: Record<string, unknown>;
  status: 'running' | 'succeeded' | 'failed' | 'timed_out';
  phase?: JsBlockRunPhase;
  view?: unknown;
  outputs: Record<string, unknown>;
  logs: JsBlockWorkerLogEntry[];
  interface_calls: JsBlockInterfaceCallTrace[];
  diagnostics: BlockRuntimeDiagnostic[];
}

export function createJsBlockDraftRun({
  state,
  requestId,
  interfaceCalls = [],
  diagnostics = []
}: {
  state: JsBlockRuntimeSessionState;
  requestId: string;
  interfaceCalls?: JsBlockInterfaceCallTrace[];
  diagnostics?: BlockRuntimeDiagnostic[];
}): JsBlockDraftRun | null {
  const request = state.requests[requestId];
  if (!request) return null;
  const result = request.result;
  return {
    run_id: requestId,
    draft_hash: hashJsBlockDraft(readRequestSource(request.request.program)),
    context_snapshot: { ...request.request.contextSnapshot },
    status:
      request.status === 'ready'
        ? 'succeeded'
        : request.status === 'timed_out'
          ? 'timed_out'
          : request.status === 'failed' || request.status === 'disposed'
            ? 'failed'
            : 'running',
    phase: request.phase,
    ...(result?.ok ? { view: result.view } : {}),
    outputs: result?.ok ? result.outputs : {},
    logs: [...request.logs],
    interface_calls: [...interfaceCalls],
    diagnostics: [...diagnostics]
  };
}

function readRequestSource(
  program: JsBlockRuntimeSessionState['requests'][string]['request']['program']
): string {
  return program.kind === 'source' ? program.source : program.fallback.source;
}

export function hashJsBlockDraft(source: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}
