import { describe, expect, test } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { toCanvasNodes } from '../to-canvas-nodes';
import { toCanvasEdges } from '../to-canvas-edges';
import { syncCanvasEdges, syncCanvasNodes } from '../sync-canvas-elements';

const noop = () => {};
const actions = {
  onOpenPicker: noop,
  onClosePicker: noop,
  onOpenContainer: noop,
  onSelectNode: noop,
  onInsertNode: noop,
  onReplaceNode: noop,
  onDeleteNode: noop,
  nodePickerOptions: []
};

describe('canvas document synchronization', () => {
  test('retains measurements and untouched nodes when LLM configuration changes', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const current = toCanvasNodes(document, null, null, null, null, {}, actions)
      .map((node) => ({ ...node, measured: { width: 196, height: 106 } }));
    const edited = {
      ...document,
      graph: {
        ...document.graph,
        nodes: document.graph.nodes.map((node) => node.id === 'node-llm'
          ? { ...node, config: { ...node.config, model_provider: {
            provider_code: 'openai_compatible', model_id: 'updated-model'
          } } }
          : node)
      }
    };
    const next = syncCanvasNodes(current,
      toCanvasNodes(edited, null, null, null, null, {}, actions));
    expect(next.find((node) => node.id === 'node-start')).toBe(current[0]);
    const before = current.find((node) => node.id === 'node-llm')!;
    const after = next.find((node) => node.id === 'node-llm')!;
    expect(after.data.config.model_provider).toEqual({
      provider_code: 'openai_compatible', model_id: 'updated-model'
    });
    expect(after.measured).toBe(before.measured);
    expect(after).not.toBe(before);
    expect(syncCanvasNodes(next,
      toCanvasNodes(edited, null, null, null, null, {}, actions))).toBe(next);
  });

  test('applies additions, deletions and type changes without inheriting old measurements', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const projected = toCanvasNodes(document, null, null, null, null, {}, actions);
    const old = { ...projected[0], measured: { width: 196, height: 106 } };
    const replacement = { ...projected[0], type: 'agentFlowContainer' as const };
    const added = { ...projected[1], id: 'new-node' };
    const next = syncCanvasNodes([old, projected[2]], [replacement, added]);
    expect(next).toEqual([replacement, added]);
    expect(next[0].measured).toBeUndefined();
  });

  test('reuses unchanged edges and still applies connection changes', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const current = toCanvasEdges(document, null, null, actions);
    expect(syncCanvasEdges(current,
      toCanvasEdges(document, null, null, actions))).toBe(current);
    const changed = { ...current[0], target: 'another-node' };
    const next = syncCanvasEdges(current, [changed]);
    expect(next).toEqual([changed]);
  });
});
