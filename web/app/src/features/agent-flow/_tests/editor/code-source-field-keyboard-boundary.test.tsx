import { ReactFlow, ReactFlowProvider } from '@xyflow/react';
import { act, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { CodeSourceField } from '../../components/detail/fields/CodeSourceField';

const monacoEditorProps = vi.hoisted(() => ({
  current: null as Record<string, unknown> | null
}));

vi.mock('@monaco-editor/react', () => ({
  default: (props: Record<string, unknown>) => {
    monacoEditorProps.current = props;
    return <div data-testid="monaco-surface" />;
  }
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

test('AC-001 inserts canonical SQL variable tokens through Monaco suggestions', async () => {
  render(
    <CodeSourceField
      label="SQL statement"
      language="sql"
      value="select "
      variableOptions={[
        {
          nodeId: 'node-start',
          nodeLabel: 'Start',
          outputKey: 'user_id',
          outputLabel: 'User ID',
          valueType: 'number',
          value: ['node-start', 'user_id'],
          displayLabel: 'Start / User ID'
        }
      ]}
      onChange={vi.fn()}
    />
  );

  await screen.findByTestId('monaco-surface');

  const registerCompletionItemProvider = vi.fn(
    (_language: string, _provider: unknown) => ({ dispose: vi.fn() })
  );
  const trigger = vi.fn();
  const model = {
    getLineContent: () => 'select {'
  };
  const editor = {
    getModel: () => model,
    focus: vi.fn(),
    trigger
  };
  const monaco = {
    languages: {
      CompletionItemKind: { Variable: 4 },
      registerCompletionItemProvider
    }
  };

  const onMount = monacoEditorProps.current?.onMount as
    | ((editor: unknown, monaco: unknown) => void)
    | undefined;
  expect(onMount).toBeTypeOf('function');
  act(() => onMount?.(editor, monaco));

  const provider = registerCompletionItemProvider.mock.calls[0]?.[1] as {
    triggerCharacters: string[];
    provideCompletionItems: (
      model: unknown,
      position: { lineNumber: number; column: number }
    ) => {
      suggestions: Array<{ insertText: string }>;
    };
  };
  expect(provider.triggerCharacters).toContain('{');
  expect(
    provider.provideCompletionItems(model, { lineNumber: 1, column: 9 })
      .suggestions[0]?.insertText
  ).toBe('{{node-start.user_id}}');

  act(() => screen.getByRole('button').click());
  expect(trigger).toHaveBeenCalledWith(
    'sql-variable-toolbar',
    'editor.action.triggerSuggest',
    undefined
  );
});
