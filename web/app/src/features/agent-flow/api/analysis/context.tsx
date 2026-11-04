import { createContext, useContext } from 'react';
import type { DocumentAnalysisState } from './use-document-analysis';

export const DocumentAnalysisContext = createContext<DocumentAnalysisState>({
  analysis: null,
  status: 'pending'
});
export const useCurrentDocumentAnalysis = () =>
  useContext(DocumentAnalysisContext);
