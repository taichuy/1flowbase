import '../../../workflow/register';
import { expect, test, vi } from 'vitest';
import { containerDocument } from './fixtures';
import { toCanvasNodes } from '../../lib/adapters/to-canvas-nodes';

const actions = {
  onOpenPicker: vi.fn(),
  onClosePicker: vi.fn(),
  onOpenContainer: vi.fn(),
  onSelectNode: vi.fn(),
  onInsertNode: vi.fn(),
  onReplaceNode: vi.fn(),
  onDeleteNode: vi.fn(),
  nodePickerOptions: []
};
const project = (document: ReturnType<typeof containerDocument>) =>
  toCanvasNodes(document, null, null, null, null, {}, actions);

test('large add control follows actual children, including disconnected nested containers', () => {
  const document = containerDocument();
  const flag = (id: string) =>
    project(document).find((node) => node.id === id)!.data.emptyContainer;
  expect(flag('outer-start')).toBe(false); // nested iteration
  expect(flag('inner-start')).toBe(false); // ordinary code
  document.graph.edges = [];
  expect(flag('outer-start')).toBe(false);
  expect(flag('inner-start')).toBe(false);
  document.graph.nodes = document.graph.nodes.filter(
    (node) => node.id !== 'leaf'
  );
  expect(flag('inner-start')).toBe(true);
  expect(flag('outer-start')).toBe(false);
});
