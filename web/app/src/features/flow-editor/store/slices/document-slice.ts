import type { ConsoleFlowVersionSummary } from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

export interface DocumentSlice {
  workingDocument: FlowAuthoringDocument;
  lastSavedDocument: FlowAuthoringDocument;
  undoStack: FlowAuthoringDocument[];
  draftMeta: {
    draftId: string;
    flowId: string;
    updatedAt: string;
  };
  versions: ConsoleFlowVersionSummary[];
}
