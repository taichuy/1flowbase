import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { getWorkflowStartNodeVariableOutputs } from '../lib/node-definitions';
import { listWorkflowVariableOptions } from '../lib/variables';
import '../register';

describe('workflow start variables', () => {
  test('exposes workflow start input fields without agent flow system variables', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
    const workflowStartNode = document.graph.nodes.find(
      (node) => node.id === 'node-workflow-start'
    );

    if (!workflowStartNode) {
      throw new Error('expected workflow start node');
    }

    workflowStartNode.config.input_fields = [
      {
        key: 'customer_id',
        label: 'Customer ID',
        inputType: 'text',
        valueType: 'string',
        required: true
      },
      {
        key: 'priority',
        label: 'Priority',
        inputType: 'number',
        valueType: 'number',
        required: false
      }
    ];

    const selectorOptions = listWorkflowVariableOptions(
      document,
      'node-workflow-end',
      { triggerType: 'schedule' }
    );
    const selectorValues = selectorOptions.map((option) => option.value);

    expect(selectorValues).toEqual(
      expect.arrayContaining([
        ['node-workflow-start', 'customer_id'],
        ['node-workflow-start', 'priority']
      ])
    );
    expect(selectorValues).not.toContainEqual([
      'node-workflow-start',
      'history'
    ]);
    expect(selectorValues).not.toContainEqual(['node-workflow-start', 'tools']);
    expect(selectorValues).toEqual(
      expect.arrayContaining([
        ['sys', 'application_id'],
        ['sys', 'workflow_id'],
        ['sys', 'workflow_run_id'],
        ['trigger', 'type'],
        ['trigger', 'scheduled_at'],
        ['trigger', 'timezone']
      ])
    );
    expect(selectorValues).not.toEqual(
      expect.arrayContaining([
        ['sys', 'conversation_id'],
        ['sys', 'dialog_count'],
        ['sys', 'user_id'],
        ['sys', 'model_parameters']
      ])
    );

    const variableOutputs =
      getWorkflowStartNodeVariableOutputs(workflowStartNode);

    expect(variableOutputs.map((output) => output.title)).toEqual([
      'input.customer_id',
      'input.priority'
    ]);
  });
});
