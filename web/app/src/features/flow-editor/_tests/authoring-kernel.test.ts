import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';

import { normalizeStartInputField } from '../authoring/start-input-fields';
import { validateAuthoringDocument } from '../authoring/validation';

describe('flow authoring kernel', () => {
  test('AC-010 parses boundary input fields without product defaults', () => {
    expect(
      normalizeStartInputField(
        {
          key: 'priority',
          label: 'Priority',
          inputType: 'number',
          required: true,
          defaultValue: 3,
          source: 'body'
        },
        0
      )
    ).toEqual(
      expect.objectContaining({
        key: 'priority',
        valueType: 'number',
        defaultValue: 3,
        source: 'body'
      })
    );
  });

  test('AC-003 validates graph integrity without product boundary rules', () => {
    const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });

    expect(validateAuthoringDocument(document)).toEqual([]);
    expect(
      validateAuthoringDocument({
        ...document,
        graph: {
          ...document.graph,
          edges: [
            ...document.graph.edges,
            {
              ...document.graph.edges[0],
              id: 'edge-dangling',
              source: 'node-workflow-start',
              target: 'missing-node'
            }
          ]
        }
      })
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'edge-dangling-dangling' })
      ])
    );
  });
});
