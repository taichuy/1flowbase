import {
  createDefaultAgentFlowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';
import { createNodeDocument } from '../../lib/document/node-factory';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import { initializeContainer } from '../../lib/document/transforms/container-geometry';
import { insertNodeAfter } from '../../lib/document/transforms/node';
export function containerDocument() {
  let document = createDefaultAgentFlowDocument({ flowId: 'containers' });
  const upstream = createNodeDocument('code', 'upstream');
  upstream.outputs = [
    {
      key: 'items',
      title: 'items',
      valueType: 'array',
      selector: ['result', 'items']
    },
    {
      key: 'count',
      title: 'count',
      valueType: 'number',
      selector: ['result', 'count']
    }
  ];
  const outer = createNodeDocument('loop', 'outer');
  outer.config = {
    ...outer.config,
    round_timeout_ms: 1000,
    loop_variables: [{ key: 'count', value_type: 'number' }]
  };
  outer.bindings.variables = {
    kind: 'named_bindings',
    value: [{ name: 'count', value: { kind: 'constant', value: 0 } }]
  };
  document.graph.nodes = [upstream, outer, createNodeDocument('code', 'after')];
  document.graph.edges = [
    createEdgeDocument({ id: 'a', source: 'upstream', target: 'outer' }),
    createEdgeDocument({ id: 'b', source: 'outer', target: 'after' })
  ];
  document = initializeContainer(document, 'outer');
  document = insertNodeAfter(
    document,
    'outer-start',
    createNodeDocument('iteration', 'inner')
  );
  document = insertNodeAfter(
    document,
    'inner-start',
    createNodeDocument('code', 'leaf')
  );
  const inner = document.graph.nodes.find((node) => node.id === 'inner')!;
  inner.config.round_timeout_ms = 500;
  inner.bindings.items = {
    kind: 'selector',
    value: ['upstream', 'result', 'items']
  };
  inner.bindings.output = {
    kind: 'selector',
    value: ['leaf', 'result', 'result']
  };
  return document;
}
export function state(document: FlowAuthoringDocument) {
  return {
    flow_id: 'containers',
    messages: [],
    draft: {
      id: 'draft',
      flow_id: 'containers',
      updated_at: '2026-10-29T00:00:00Z',
      document
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10
  };
}
