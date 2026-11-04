import type { ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import { useMemo, type PropsWithChildren } from 'react';
import { DocumentAnalysisContext } from '../../../api/analysis/context';
import { AgentFlowEditorStoreProvider as EditorStoreProvider } from '../../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../../store/editor/provider';
import { historicalAnalysisFixture } from '../../fixtures/analysis-fixture';

function InspectorAnalysisFixture({ children }: PropsWithChildren) {
  const document = useAgentFlowEditorStore((state) => state.workingDocument);
  const analysis = useMemo(
    () => historicalAnalysisFixture(document),
    [document]
  );
  const value = useMemo(
    () => ({ analysis, status: 'ready' as const }),
    [analysis]
  );
  return (
    <DocumentAnalysisContext.Provider value={value}>
      {children}
    </DocumentAnalysisContext.Provider>
  );
}

export function AgentFlowEditorStoreProvider({
  initialState,
  children
}: PropsWithChildren<{ initialState: ConsoleApplicationOrchestrationState }>) {
  return (
    <EditorStoreProvider initialState={initialState}>
      <InspectorAnalysisFixture>{children}</InspectorAnalysisFixture>
    </EditorStoreProvider>
  );
}
