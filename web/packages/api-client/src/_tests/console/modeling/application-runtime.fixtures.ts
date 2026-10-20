import { afterEach, expect, test, vi } from 'vitest';

import {
  getConsoleApplicationRunDebugSnapshot,
  completeConsoleRunArchiveUploadSession,
  createConsoleApplicationRunsArchive,
  createConsoleRunArchiveUploadSession,
  getConsoleApplicationConversationMessages,
  getConsoleApplicationRunArchive,
  getConsoleApplicationRunConversationMessages,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRuntimeActivity,
  getConsoleApplicationRunNodeLastRun,
  getConsoleApplicationRunResumeTimeline,
  getConsoleApplicationRunResumeTimelineSummary,
  getConsoleApplicationRuns,
  getConsoleApplicationRunTraceNodeChildren,
  getConsoleApplicationRunTraceNodeContent,
  getConsoleApplicationRunTraceNodeDetail,
  getConsoleApplicationRunTraceToolCallbackContent,
  getConsoleApplicationRunTraceTree,
  exportConsoleApplicationRunTraceDump,
  exportConsoleApplicationRunsTraceDumpZip,
  getConsoleDebugVariableSnapshot,
  getConsoleRuntimeDebugStream,
  getConsoleRuntimeDebugArtifact,
  normalizeConsoleRuntimeEvent,
  getConsoleRunArchiveImportJob,
  resolveConsoleRuntimeDebugArtifacts,
  startConsoleFlowDebugRunStream,
  subscribeConsoleFlowDebugRunStream,
  uploadConsoleRunArchiveChunk
} from '../../../console/application-runtime';

function sseResponse(frame: string) {
  return new Response(
    new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(frame));
        controller.close();
      }
    }),
    {
      status: 200,
      headers: { 'content-type': 'text/event-stream' }
    }
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

export {
  completeConsoleRunArchiveUploadSession,
  createConsoleApplicationRunsArchive,
  createConsoleRunArchiveUploadSession,
  expect,
  exportConsoleApplicationRunTraceDump,
  exportConsoleApplicationRunsTraceDumpZip,
  getConsoleApplicationConversationMessages,
  getConsoleApplicationRunArchive,
  getConsoleApplicationRunConversationMessages,
  getConsoleApplicationRunDebugSnapshot,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRunNodeLastRun,
  getConsoleApplicationRunResumeTimeline,
  getConsoleApplicationRunResumeTimelineSummary,
  getConsoleApplicationRunTraceNodeChildren,
  getConsoleApplicationRunTraceNodeContent,
  getConsoleApplicationRunTraceNodeDetail,
  getConsoleApplicationRunTraceToolCallbackContent,
  getConsoleApplicationRunTraceTree,
  getConsoleApplicationRuns,
  getConsoleApplicationRuntimeActivity,
  getConsoleDebugVariableSnapshot,
  getConsoleRunArchiveImportJob,
  getConsoleRuntimeDebugArtifact,
  getConsoleRuntimeDebugStream,
  normalizeConsoleRuntimeEvent,
  resolveConsoleRuntimeDebugArtifacts,
  sseResponse,
  startConsoleFlowDebugRunStream,
  subscribeConsoleFlowDebugRunStream,
  test,
  uploadConsoleRunArchiveChunk,
  vi
};
