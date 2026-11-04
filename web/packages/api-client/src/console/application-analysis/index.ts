import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { apiFetch } from '../../transport';

export interface FlowAnalysisDiagnostic {
  node_id: string | null;
  code: string;
  level: 'error' | 'warning';
  field_path: string;
  message: string;
  selector: string[] | null;
}
export interface FlowResolvedOutput {
  key: string;
  title: string;
  valueType: string;
  selector: string[];
  jsonSchema: Record<string, unknown> | null;
}
export interface FlowNodeOutputs {
  node_id: string;
  outputs: FlowResolvedOutput[];
}
export interface FlowVariableOption {
  node_id: string;
  node_label: string;
  output_key: string;
  output_label: string;
  value_type: string;
  json_schema: Record<string, unknown> | null;
  selector: string[];
}
export interface FlowVariableCatalog {
  node_id: string;
  field_path: string;
  variables: FlowVariableOption[];
}
export interface FlowDocumentAnalysis {
  diagnostics: FlowAnalysisDiagnostic[];
  node_outputs: FlowNodeOutputs[];
  variable_catalogs: FlowVariableCatalog[];
}
export function analyzeConsoleApplicationDocument(
  applicationId: string,
  input: { document: FlowAuthoringDocument; node_id?: string },
  baseUrl?: string,
  signal?: AbortSignal
): Promise<FlowDocumentAnalysis> {
  return apiFetch<FlowDocumentAnalysis>({
    path: `/api/console/applications/${applicationId}/orchestration/analysis`,
    method: 'POST',
    body: input,
    baseUrl,
    signal
  });
}
