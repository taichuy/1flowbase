import type { FlowDocumentAnalysis } from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { findInspectorSectionKey } from '../../lib/node-definitions';
import type { FlowSelectorOption } from '../../lib/selector-options';
import type { AgentFlowIssue } from '../../lib/validation/issues';
import { formatNodeVariableLabel } from '../../lib/variables/variable-labels';

function pointerSegments(path: string) {
  return path
    .split('/')
    .slice(1)
    .map((part) => part.replace(/~1/g, '/').replace(/~0/g, '~'));
}

export function analysisIssues(
  analysis: FlowDocumentAnalysis | null,
  document: FlowAuthoringDocument
): AgentFlowIssue[] {
  return (analysis?.diagnostics ?? []).map((diagnostic, index) => {
    const node = document.graph.nodes.find(
      (item) => item.id === diagnostic.node_id
    );
    const segments = pointerSegments(diagnostic.field_path);
    const fieldKey =
      segments[0] === 'outputs'
        ? 'config.output_contract'
        : segments.slice(0, 2).join('.') || null;
    return {
      id: `${diagnostic.code}:${diagnostic.node_id ?? 'document'}:${diagnostic.field_path}:${index}`,
      nodeId: diagnostic.node_id,
      scope: diagnostic.node_id ? (fieldKey ? 'field' : 'node') : 'global',
      level: diagnostic.level,
      fieldKey,
      sectionKey:
        node && fieldKey ? findInspectorSectionKey(node.type, fieldKey) : null,
      title: diagnostic.message,
      message: diagnostic.message
    };
  });
}

export function analysisSelectorOptions(
  analysis: FlowDocumentAnalysis | null,
  nodeId: string,
  fieldKey = ''
): FlowSelectorOption[] {
  const fieldPath = fieldKey
    ? '/' +
      fieldKey
        .split('.')
        .map((part) => part.replace(/~/g, '~0').replace(/\//g, '~1'))
        .join('/')
    : '';
  const catalogs = analysis?.variable_catalogs ?? [];
  const catalog =
    catalogs.find(
      (item) => item.node_id === nodeId && item.field_path === fieldPath
    ) ??
    catalogs.find((item) => item.node_id === nodeId && item.field_path === '');
  return (catalog?.variables ?? []).map((variable) => ({
    nodeId: variable.node_id,
    nodeLabel: variable.node_label,
    outputKey: variable.output_key,
    outputLabel: variable.output_label,
    valueType: variable.value_type,
    jsonSchema: variable.json_schema ?? undefined,
    value: variable.selector,
    displayLabel: formatNodeVariableLabel(
      variable.node_label,
      variable.output_label
    )
  }));
}
