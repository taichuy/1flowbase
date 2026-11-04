import { describe, expect, test, vi } from 'vitest';
import type { FlowDocumentAnalysis } from '@1flowbase/api-client';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { createAgentFlowNodeSchemaAdapter } from '../../../schema/node-schema-adapter';
import { analysisIssues, analysisSelectorOptions } from '../presentation';

const schema = {
  type: 'object',
  properties: {
    report: { type: 'object', properties: { score: { type: 'number' } } }
  }
};
const variable = {
  node_id: 'node-code',
  node_label: 'Compute',
  output_key: 'score',
  output_label: 'Score',
  value_type: 'number',
  json_schema: { type: 'number' },
  selector: ['node-code', 'result', 'report', 'score']
};
const analysis: FlowDocumentAnalysis = {
  diagnostics: [
    {
      node_id: 'node-llm',
      code: 'dynamic_json_path',
      level: 'warning',
      field_path: '/bindings/prompt/value',
      message: 'The field is resolved at runtime.',
      selector: ['node-code', 'result', 'report', 'score']
    }
  ],
  node_outputs: [
    {
      node_id: 'node-llm',
      outputs: [
        {
          key: 'all',
          title: 'All',
          valueType: 'object',
          selector: ['result'],
          jsonSchema: schema
        }
      ]
    }
  ],
  variable_catalogs: [
    {
      node_id: 'node-llm',
      field_path: '',
      variables: [
        {
          ...variable,
          output_key: 'whole',
          selector: ['node-code', 'result'],
          value_type: 'object',
          json_schema: schema
        }
      ]
    },
    {
      node_id: 'node-llm',
      field_path: '/bindings/prompt',
      variables: [variable]
    }
  ]
};

describe('backend analysis consumption', () => {
  test('uses field-specific catalogs, preserving the complete backend selector and schema', () => {
    const options = analysisSelectorOptions(
      analysis,
      'node-llm',
      'bindings.prompt'
    );
    expect(options).toHaveLength(1);
    expect(options[0]).toMatchObject({
      value: variable.selector,
      valueType: 'number',
      jsonSchema: { type: 'number' }
    });
    expect(
      analysisSelectorOptions(analysis, 'node-llm', 'bindings.other')[0]
    ).toMatchObject({ value: ['node-code', 'result'], jsonSchema: schema });
    expect(analysisSelectorOptions(null, 'node-llm')).toEqual([]);
  });

  test('inspector uses server outputs and catalogs even when the local graph has no matching upstream node', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const adapter = createAgentFlowNodeSchemaAdapter({
      document,
      nodeId: 'node-llm',
      analysis,
      setWorkingDocument: vi.fn(),
      dispatch: vi.fn()
    });
    expect(adapter.getDerived('selectorOptions:bindings.prompt')).toEqual(
      analysisSelectorOptions(analysis, 'node-llm', 'bindings.prompt')
    );
    expect(adapter.getDerived('outputContract')).toEqual(
      analysis.node_outputs[0].outputs
    );
    const pending = createAgentFlowNodeSchemaAdapter({
      document,
      nodeId: 'node-llm',
      setWorkingDocument: vi.fn(),
      dispatch: vi.fn()
    });
    expect(pending.getDerived('selectorOptions')).toEqual([]);
    expect(pending.getDerived('outputContract')).toEqual([]);
  });

  test('preserves warning severity and focuses the binding field instead of inventing a local error', () => {
    const issues = analysisIssues(
      analysis,
      createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    );
    expect(issues).toHaveLength(1);
    expect(issues[0]).toMatchObject({
      level: 'warning',
      nodeId: 'node-llm',
      fieldKey: 'bindings.prompt',
      message: analysis.diagnostics[0].message
    });
  });
});
