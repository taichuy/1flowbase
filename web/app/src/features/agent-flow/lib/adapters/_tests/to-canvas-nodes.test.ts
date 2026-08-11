import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test, vi } from 'vitest';

import { toCanvasNodes } from '../to-canvas-nodes';

describe('toCanvasNodes dimensions', () => {
  test('leaves node height and measurements to React Flow DOM observation', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const nodes = toCanvasNodes(document, null, null, null, null, {}, {
      nodePickerOptions: [],
      onOpenPicker: vi.fn(),
      onClosePicker: vi.fn(),
      onOpenContainer: vi.fn(),
      onSelectNode: vi.fn(),
      onInsertNode: vi.fn(),
      onReplaceNode: vi.fn(),
      onDeleteNode: vi.fn()
    });

    expect(nodes.length).toBeGreaterThan(0);

    for (const node of nodes) {
      expect(node.height).toBeUndefined();
      expect(node.measured).toBeUndefined();
    }
  });
});
