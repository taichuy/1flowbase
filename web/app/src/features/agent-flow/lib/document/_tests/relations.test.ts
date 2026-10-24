import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { getDirectDownstreamNodes } from '../relations';

describe('getDirectDownstreamNodes', () => {
  test('preserves node order and identity while merging edges and ignoring missing targets', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'downstream' });
    const [first] = document.graph.edges;
    const [start, llm, answer] = document.graph.nodes;
    document.graph.nodes = [answer, start, llm];
    document.graph.edges = [
      first,
      { ...first, id: 'duplicate' },
      { ...first, id: 'answer', target: answer.id, containerId: 'nested' },
      { ...first, id: 'missing', target: 'missing' },
      { ...first, id: 'unrelated', source: 'other', target: start.id }
    ];

    const downstream = getDirectDownstreamNodes(document, start.id);

    expect(downstream).toEqual([answer, llm]);
    expect(downstream[0]).toBe(answer);
    expect(downstream[1]).toBe(llm);
    expect(getDirectDownstreamNodes(document, 'absent')).toEqual([]);
  });

  test('keeps every matching node record, including duplicate IDs and self loops', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'self-loop' });
    const [first] = document.graph.edges;
    const [start] = document.graph.nodes;
    const duplicate = { ...start, containerId: 'nested' };
    document.graph.nodes.push(duplicate);
    document.graph.edges = [{ ...first, target: start.id }];

    expect(getDirectDownstreamNodes(document, start.id)).toEqual([
      start,
      duplicate
    ]);
  });
});
