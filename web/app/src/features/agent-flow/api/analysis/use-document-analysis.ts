import type { FlowDocumentAnalysis } from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { useEffect, useMemo, useState } from 'react';
import { analyzeDocument } from '../orchestration';

export interface DocumentAnalysisState {
  analysis: FlowDocumentAnalysis | null;
  status: 'pending' | 'ready' | 'error';
}

export function useDocumentAnalysis(
  applicationId: string,
  document: FlowAuthoringDocument,
  selectedNodeId: string | null = null,
  environmentVariables?: unknown,
  triggerContext?: unknown
): DocumentAnalysisState {
  const node_id =
    selectedNodeId &&
    document.graph.nodes.some((node) => node.id === selectedNodeId)
      ? selectedNodeId
      : undefined;
  const request = useMemo(
    () => ({
      applicationId,
      document,
      node_id,
      environmentVariables,
      triggerContext
    }),
    [applicationId, document, node_id, environmentVariables, triggerContext]
  );
  const [result, setResult] = useState<{
    request: typeof request;
    analysis: FlowDocumentAnalysis | null;
    status: 'ready' | 'error';
  } | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    const timer = window.setTimeout(() => {
      void analyzeDocument(
        applicationId,
        document,
        controller.signal,
        node_id
      ).then(
        (analysis) => {
          if (active) setResult({ request, analysis, status: 'ready' });
        },
        () => {
          if (active) setResult({ request, analysis: null, status: 'error' });
        }
      );
    }, 300);
    return () => {
      active = false;
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [applicationId, document, node_id, request]);

  return result?.request === request
    ? result
    : { analysis: null, status: 'pending' };
}
