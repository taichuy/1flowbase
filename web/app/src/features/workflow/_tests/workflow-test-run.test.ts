import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { buildWorkflowTestRunInput } from '../lib/test-run';

function createWorkflowDocument() {
  const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  )!;
  startNode.config.input_fields = [
    {
      key: 'customer_id',
      label: 'Customer ID',
      inputType: 'text',
      valueType: 'string',
      required: true,
      source: 'path'
    },
    {
      key: 'force',
      label: 'Force',
      inputType: 'checkbox',
      valueType: 'boolean',
      required: false,
      defaultValue: false,
      source: 'query'
    },
    {
      key: 'limit',
      label: 'Limit',
      inputType: 'number',
      valueType: 'number',
      required: false,
      defaultValue: 100
    }
  ];
  return document;
}

describe('Workflow test-run input', () => {
  test('keys parameters by the start node and ignores undeclared input', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        inputs: {
          customer_id: 'C-42',
          force: true,
          limit: 25,
          undeclared: 'ignored'
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': { customer_id: 'C-42', force: true, limit: 25 }
      }
    });
  });

  test('applies start field defaults and preserves false and zero', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        inputs: { customer_id: 'C-42', limit: 0 }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': { customer_id: 'C-42', force: false, limit: 0 }
      }
    });
  });

  test('omits an optional parameter without a value or default', () => {
    const document = createWorkflowDocument();
    document.graph.nodes[0].config.input_fields = [
      {
        key: 'note',
        label: 'Note',
        inputType: 'text',
        valueType: 'string',
        required: false
      }
    ];
    expect(buildWorkflowTestRunInput({ document, inputs: {} })).toEqual({
      input_payload: { 'node-workflow-start': {} }
    });
  });

  test('an empty start form sends an empty node payload', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createDefaultWorkflowDocument({ flowId: 'flow-1' }),
        inputs: { undeclared: 'ignored' }
      })
    ).toEqual({ input_payload: { 'node-workflow-start': {} } });
  });

  test('a missing start node is not treated as an empty input form', () => {
    const document = createWorkflowDocument();
    document.graph.nodes = document.graph.nodes.filter(
      (node) => node.type !== 'workflow_start'
    );
    expect(() => buildWorkflowTestRunInput({ document, inputs: {} })).toThrow(
      'Workflow document requires a workflow_start node'
    );
  });
});
