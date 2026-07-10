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
      key: 'payload',
      label: 'Payload',
      inputType: 'json',
      valueType: 'object',
      required: false,
      source: 'body'
    },
    {
      key: 'attachment',
      label: 'Attachment',
      inputType: 'text',
      valueType: 'string',
      required: false,
      defaultValue: 'none',
      source: 'form'
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
  test('AC-102 builds schedule payload under the workflow_start node id', () => {
    const document = createWorkflowDocument();
    const startNode = document.graph.nodes.find(
      (node) => node.type === 'workflow_start'
    );
    if (!startNode || !Array.isArray(startNode.config.input_fields)) {
      throw new Error('workflow_start input fields are missing');
    }
    startNode.config.input_fields = startNode.config.input_fields.map((field) => {
      const scheduleField = { ...field };
      delete scheduleField.source;
      return scheduleField;
    });

    expect(
      buildWorkflowTestRunInput({
        document,
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

  test('AC-102 maps all HTTP sources directly from workflow_start input fields', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'extension',
        extensionInputs: {
          path: { customer_id: 'C-42', undeclared: 'ignored' },
          query: { force: true },
          body: { payload: { ticket: 'T-1' } },
          form: { attachment: 'invoice.pdf' }
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': {
          customer_id: 'C-42',
          force: true,
          payload: { ticket: 'T-1' },
          attachment: 'invoice.pdf'
        }
      }
    });
  });

  test('AC-102 applies declared defaults without creating undeclared inputs', () => {
    expect(
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'extension',
        extensionInputs: {
          path: { customer_id: 'C-42' },
          query: {},
          body: {},
          form: {}
        }
      })
    ).toEqual({
      input_payload: {
        'node-workflow-start': {
          customer_id: 'C-42',
          force: false,
          attachment: 'none'
        }
      }
    });
  });

  test('AC-102 rejects a missing required workflow_start input', () => {
    expect(() =>
      buildWorkflowTestRunInput({
        document: createWorkflowDocument(),
        triggerType: 'extension',
        extensionInputs: {
          path: {},
          query: {},
          body: {},
          form: {}
        }
      })
    ).toThrow('Missing required workflow input: customer_id');
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
