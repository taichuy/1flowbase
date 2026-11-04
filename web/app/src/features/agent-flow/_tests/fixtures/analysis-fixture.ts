import type {
  FlowDocumentAnalysis,
  FlowVariableOption
} from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import {
  listLlmProtocolContextSelectorOptions,
  listVisibleSelectorOptions,
  type FlowSelectorOption
} from '../../lib/selector-options';
import { getNodeVariableOutputs } from '../../lib/variables/start-node-variables';
import { validateDocument } from '../../lib/validate-document';

function variable(option: FlowSelectorOption): FlowVariableOption {
  return {
    node_id: option.nodeId,
    node_label: option.nodeLabel,
    output_key: option.outputKey,
    output_label: option.outputLabel,
    value_type: option.valueType,
    json_schema: option.jsonSchema ?? null,
    selector: option.value
  };
}

/** Synthetic server fixtures for historical editor interaction scenarios only.
 * Backend authority is tested with explicit DTOs in api/analysis/_tests.
 */
export function historicalAnalysisFixture(
  document: FlowAuthoringDocument
): FlowDocumentAnalysis {
  return {
    diagnostics: validateDocument(document).map((issue) => ({
      node_id: issue.nodeId,
      code: issue.id,
      level: issue.level,
      field_path: issue.fieldKey
        ? '/' + issue.fieldKey.split('.').join('/')
        : '',
      message: issue.title,
      selector: null
    })),
    node_outputs: document.graph.nodes.map((node) => ({
      node_id: node.id,
      outputs: getNodeVariableOutputs(node).map((output) => ({
        ...output,
        selector: output.selector ?? [output.key],
        jsonSchema: output.jsonSchema ?? null
      }))
    })),
    variable_catalogs: document.graph.nodes.flatMap((node) => [
      {
        node_id: node.id,
        field_path: '',
        variables: listVisibleSelectorOptions(document, node.id).map(variable)
      },
      ...Object.keys(node.bindings).map((key) => ({
        node_id: node.id,
        field_path: '/bindings/' + key.replace(/~/g, '~0').replace(/\//g, '~1'),
        variables: listVisibleSelectorOptions(
          document,
          node.id,
          [],
          null,
          'bindings.' + key
        ).map(variable)
      })),
      {
        node_id: node.id,
        field_path: '/config/protocol_context',
        variables: listLlmProtocolContextSelectorOptions(document, node.id).map(
          variable
        )
      }
    ])
  };
}
