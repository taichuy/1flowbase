import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { validateWorkflowDocument } from '../lib/validate-document';

describe('validateWorkflowDocument', () => {
  test('AC-005 accepts one workflow start and at least one workflow end', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });

    expect(validateWorkflowDocument(document)).toEqual([]);
  });

  test('AC-004 rejects AgentFlow boundary nodes without requiring Start or Answer', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
    document.graph.nodes.push({
      ...document.graph.nodes[0],
      id: 'node-agent-start',
      type: 'start',
      alias: 'Start'
    });

    const issues = validateWorkflowDocument(document);

    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'workflow-node-family-node-agent-start',
          level: 'error'
        })
      ])
    );
    expect(issues.some((issue) => issue.id === 'global-start-count')).toBe(
      false
    );
    expect(issues.some((issue) => issue.id === 'global-answer-missing')).toBe(
      false
    );
  });

  test('AC-005 requires exactly one workflow start and at least one workflow end', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.filter(
      (node) => node.type !== 'workflow_end'
    );
    document.graph.nodes.push({
      ...document.graph.nodes[0],
      id: 'node-workflow-start-2'
    });

    expect(validateWorkflowDocument(document)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'workflow-start-count' }),
        expect.objectContaining({ id: 'workflow-end-count' })
      ])
    );
  });
});
