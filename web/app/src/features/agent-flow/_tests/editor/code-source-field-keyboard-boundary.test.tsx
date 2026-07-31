import { ReactFlow, ReactFlowProvider } from '@xyflow/react';
import { act, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { CodeSourceField } from '../../components/detail/fields/CodeSourceField';

vi.mock('@monaco-editor/react', () => ({
  default: () => <div data-testid="monaco-surface" />
}));

test('AC-001/AC-002 scopes Space to Monaco without disabling the canvas shortcut', async () => {
  render(
    <ReactFlowProvider>
      <ReactFlow edges={[]} nodes={[]}>
        <CodeSourceField label="SQL statement" value="select" onChange={vi.fn()} />
      </ReactFlow>
    </ReactFlowProvider>
  );

  const editorSurface = await screen.findByTestId('monaco-surface');
  const spaceEvent = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    code: 'Space',
    key: ' '
  });

  act(() => editorSurface.dispatchEvent(spaceEvent));

  expect(spaceEvent.defaultPrevented).toBe(false);

  const canvasSpaceEvent = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    code: 'Space',
    key: ' '
  });

  act(() =>
    screen.getByRole('application').dispatchEvent(canvasSpaceEvent)
  );

  expect(canvasSpaceEvent.defaultPrevented).toBe(true);
});
