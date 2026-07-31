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
        <CodeSourceField
          label="SQL statement"
          value="select"
          onChange={vi.fn()}
        />
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

  act(() => screen.getByRole('application').dispatchEvent(canvasSpaceEvent));

  expect(canvasSpaceEvent.defaultPrevented).toBe(true);
});

test('AC-001/AC-002 filters SQL variable queries and replaces the full trigger fragment', async () => {
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
        },
        {
          nodeId: 'node-start',
          nodeLabel: 'Start',
          outputKey: 'model',
          outputLabel: 'model',
          valueType: 'string',
          value: ['node-start', 'model'],
          displayLabel: 'Start/model'
        },
        {
          nodeId: 'node-start',
          nodeLabel: 'Start',
          outputKey: 'system',
          outputLabel: 'system',
          valueType: 'string',
          value: ['node-start', 'system'],
          displayLabel: 'Start/system'
        },
        {
          nodeId: 'sys',
          nodeLabel: 'System variables',
          outputKey: 'user_id',
          outputLabel: 'sys.user_id',
          valueType: 'string',
          value: ['sys', 'user_id'],
          displayLabel: 'sys.user_id'
        },
        {
          nodeId: 'sys',
          nodeLabel: 'System variables',
          outputKey: 'model_parameters',
          outputLabel: 'sys.model_parameters',
          valueType: 'json',
          value: ['sys', 'model_parameters'],
          displayLabel: 'sys.model_parameters'
        },
        {
          nodeId: 'node-start',
          nodeLabel: 'Start',
          outputKey: 'reasoning_effort',
          outputLabel: 'reasoning_effort',
          valueType: 'string',
          value: ['node-start', 'reasoning_effort'],
          displayLabel: 'Start/reasoning_effort'
        }
      ]}
      onChange={vi.fn()}
    />
  );

  await screen.findByTestId('monaco-surface');

  const registerCompletionItemProvider = vi.fn<
    (language: string, provider: unknown) => { dispose: () => void }
  >(() => ({ dispose: vi.fn() }));
  const trigger = vi.fn();
  let modelLine = 'select {';
  const model = {
    getLineContent: () => modelLine
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
      suggestions: Array<{
        insertText: string;
        label: string;
        range: {
          startLineNumber: number;
          endLineNumber: number;
          startColumn: number;
          endColumn: number;
        };
      }>;
    };
  };
  expect(provider.triggerCharacters).toContain('{');
  expect(
    provider.provideCompletionItems(model, { lineNumber: 1, column: 9 })
      .suggestions[0]?.insertText
  ).toBe('{{node-start.user_id}}');

  modelLine = 'select {sy}';
  const partialSuggestions = provider.provideCompletionItems(model, {
    lineNumber: 1,
    column: 11
  }).suggestions;
  expect(partialSuggestions.map((suggestion) => suggestion.label)).toEqual([
    'sys.user_id',
    'sys.model_parameters',
    'Start/system'
  ]);
  expect(partialSuggestions[0]?.range).toEqual({
    startLineNumber: 1,
    endLineNumber: 1,
    startColumn: 8,
    endColumn: 12
  });

  act(() => screen.getByRole('button').click());
  expect(trigger).toHaveBeenCalledWith(
    'sql-variable-toolbar',
    'editor.action.triggerSuggest',
    undefined
  );
});
