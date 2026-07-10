import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import {
  buildWorkflowTestRunInput,
  readWorkflowResult
} from '../lib/test-run';

function createWorkflowDocument() {
  const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  );
  const endNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_end'
  );

  if (!startNode || !endNode) {
    throw new Error('default workflow nodes are missing');
  }

  startNode.config.input_fields = [
    {
      key: 'customer_id',
      label: 'Customer ID',
      inputType: 'text',
      valueType: 'string',
      required: true
    },
    {
      key: 'force',
      label: 'Force',
      inputType: 'checkbox',
      valueType: 'boolean',
      required: false,
      defaultValue: false
    }
  ];
  endNode.outputs = [
    {
      key: 'ticket_id',
      title: 'Ticket ID',
      valueType: 'string'
    }
  ];

  return document;
}

describe('Workflow test-run contract', () => {
  test('AC-102 builds manual inputs under the workflow_start node id', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'manual',
        manualInputs: {
          customer_id: 'C-42',
          force: true
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': {
          customer_id: 'C-42',
          force: true
        }
      }
    });
  });

  test('AC-102 builds schedule payload under the workflow_start node id', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'schedule',
        schedulePayload: {
          customer_id: 'C-42',
          force: false
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': {
          customer_id: 'C-42',
          force: false
        }
      }
    });
  });

  test('AC-102 maps extension source values to workflow_start targets', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'extension',
        extensionParameters: [
          {
            name: 'customerId',
            source: 'path',
            target: 'node-workflow-start.customer_id'
          },
          {
            name: 'force',
            source: 'query',
            target: 'node-workflow-start.force'
          }
        ],
        extensionInputs: {
          path: { customerId: 'C-42' },
          query: { force: true },
          form: {},
          body: {}
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': {
          customer_id: 'C-42',
          force: true
        }
      }
    });
  });

  test('AC-104 reads Workflow Result from flow_run.output_payload', () => {
    expect(
      readWorkflowResult({
        flow_run: {
          output_payload: {
            ticket_id: 'ticket-C-42'
          }
        }
      })
    ).toEqual({ ticket_id: 'ticket-C-42' });
  });
});
