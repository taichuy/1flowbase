import { expect, test } from 'vitest';
import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';
import { containerDocument } from '../../agent-flow/_tests/containers/fixtures';
import { createEdgeDocument } from '../../agent-flow/lib/document/edge-factory';
import { validateWorkflowDocument } from '../lib/validate-document';
import { listWorkflowVariableOptions } from '../lib/variables';
import '../register';

test('Workflow validates the same lexical container scopes as its inspector', () => {
  const document = containerDocument();
  const defaults = createDefaultWorkflowDocument({ flowId: 'containers' });
  document.graph.nodes.push(...defaults.graph.nodes);
  document.graph.edges.push(
    createEdgeDocument({
      id: 'workflow-input',
      source: 'node-workflow-start',
      target: 'upstream'
    }),
    createEdgeDocument({
      id: 'workflow-output',
      source: 'after',
      target: 'node-workflow-end'
    })
  );
  expect(
    listWorkflowVariableOptions(document, 'inner', null, 'bindings.output').map(
      (option) => option.value
    )
  ).toContainEqual(['leaf', 'result', 'result']);
  expect(
    listWorkflowVariableOptions(document, 'inner', null, 'bindings.items').map(
      (option) => option.value
    )
  ).toContainEqual(['upstream', 'result', 'items']);
  expect(
    validateWorkflowDocument(document).filter((issue) =>
      issue.id.endsWith('selector-not-visible')
    )
  ).toEqual([]);
  document.graph.nodes.find((node) => node.id === 'after')!.bindings.value = {
    kind: 'selector',
    value: ['leaf', 'result', 'result']
  };
  expect(validateWorkflowDocument(document)).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ id: 'after-value-selector-not-visible' })
    ])
  );
});
