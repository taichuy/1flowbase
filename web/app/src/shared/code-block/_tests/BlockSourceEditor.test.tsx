import { act, render, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { BlockSourceEditor } from '../BlockSourceEditor';

interface EditorHarnessProps {
  beforeMount?: (monaco: unknown) => void;
  onChange?: (value?: string) => void;
  onMount?: (editor: unknown, monaco: unknown) => void;
}

const editorHarness = vi.hoisted(() => ({
  props: null as EditorHarnessProps | null
}));

vi.mock('@monaco-editor/react', () => ({
  default: (props: EditorHarnessProps) => {
    editorHarness.props = props;
    return <div />;
  }
}));

vi.mock('../monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

test('AC-001 accepts React default imports supported by the Native React runtime', async () => {
  const setCompilerOptions = vi.fn();
  const monaco = {
    languages: {
      typescript: {
        JsxEmit: { Preserve: 'preserve' },
        ModuleResolutionKind: { NodeJs: 'node-js' },
        ScriptTarget: { ES2022: 'es2022' },
        typescriptDefaults: { setCompilerOptions }
      }
    }
  };

  render(
    <BlockSourceEditor
      ariaLabel="TSX source"
      path="file:///block.tsx"
      value="import React from 'react';"
      onChange={vi.fn()}
    />
  );
  await waitFor(() => expect(editorHarness.props).not.toBeNull());
  act(() => editorHarness.props?.beforeMount?.(monaco));

  expect(setCompilerOptions).toHaveBeenCalledWith(
    expect.objectContaining({
      allowSyntheticDefaultImports: true,
      esModuleInterop: true
    })
  );
});

test('registers Monaco declarations that arrive after the editor mounts', async () => {
  const dispose = vi.fn();
  const addExtraLib = vi.fn(() => ({ dispose }));
  const monaco = {
    MarkerSeverity: { Error: 8 },
    editor: { setModelMarkers: vi.fn() },
    languages: {
      typescript: {
        JsxEmit: { Preserve: 'preserve' },
        ModuleResolutionKind: { NodeJs: 'node-js' },
        ScriptTarget: { ES2022: 'es2022' },
        typescriptDefaults: {
          addExtraLib,
          setCompilerOptions: vi.fn()
        }
      }
    }
  };
  const props = {
    ariaLabel: 'TSX source',
    path: 'file:///block.tsx',
    readOnly: false,
    value: 'export default {};',
    onChange: vi.fn()
  };
  const view = render(<BlockSourceEditor {...props} extraLibs={[]} />);

  act(() => {
    editorHarness.props?.beforeMount?.(monaco);
    editorHarness.props?.onMount?.({ getModel: () => null }, monaco);
  });
  expect(addExtraLib).not.toHaveBeenCalled();

  view.rerender(
    <BlockSourceEditor
      {...props}
      extraLibs={[
        {
          source: '@1flowbase/block-sdk',
          filePath: 'file:///node_modules/@1flowbase/block-sdk/index.d.ts',
          content: "declare module '@1flowbase/block-sdk' {}"
        }
      ]}
    />
  );

  await waitFor(() =>
    expect(addExtraLib).toHaveBeenCalledWith(
      "declare module '@1flowbase/block-sdk' {}",
      'file:///node_modules/@1flowbase/block-sdk/index.d.ts'
    )
  );
});

test('AC-001 attributes source changes to the current Monaco document path', () => {
  const onChange = vi.fn();
  const model = {
    uri: {
      toString: () => 'file:///frontstage/page-1/blocks/child.tsx'
    }
  };
  const editor = { getModel: () => model };
  const monaco = {
    MarkerSeverity: { Error: 8 },
    editor: { setModelMarkers: vi.fn() },
    languages: {
      typescript: {
        JsxEmit: { Preserve: 'preserve' },
        ModuleResolutionKind: { NodeJs: 'node-js' },
        ScriptTarget: { ES2022: 'es2022' },
        typescriptDefaults: {
          addExtraLib: vi.fn(() => ({ dispose: vi.fn() })),
          setCompilerOptions: vi.fn()
        }
      }
    }
  };

  render(
    <BlockSourceEditor
      ariaLabel="TSX source"
      path="file:///frontstage/page-1/blocks/root.tsx"
      value="root source"
      onChange={onChange}
    />
  );
  act(() => {
    editorHarness.props?.onMount?.(editor, monaco);
    editorHarness.props?.onChange?.('');
  });

  expect(onChange).toHaveBeenCalledWith(
    '',
    'file:///frontstage/page-1/blocks/child.tsx'
  );
});

test('projects source diagnostics into Monaco markers and clears them', async () => {
  const setModelMarkers = vi.fn();
  const model = {};
  const editor = { getModel: () => model };
  const monaco = {
    MarkerSeverity: { Error: 8 },
    editor: { setModelMarkers },
    languages: {
      typescript: {
        JsxEmit: { Preserve: 'preserve' },
        ModuleResolutionKind: { NodeJs: 'node-js' },
        ScriptTarget: { ES2022: 'es2022' },
        typescriptDefaults: {
          addExtraLib: vi.fn(() => ({ dispose: vi.fn() })),
          setCompilerOptions: vi.fn()
        }
      }
    }
  };
  const props = {
    ariaLabel: 'TSX source',
    path: 'file:///block.tsx',
    readOnly: false,
    value: "import value from 'dayjs';",
    onChange: vi.fn()
  };
  const view = render(
    <BlockSourceEditor
      {...props}
      diagnostics={[
        {
          code: 'import_denied',
          message: "Import source 'dayjs' is not allowed.",
          sourceLocation: { line: 1, column: 1 }
        }
      ]}
    />
  );

  act(() => {
    editorHarness.props?.beforeMount?.(monaco);
    editorHarness.props?.onMount?.(editor, monaco);
  });
  expect(setModelMarkers).toHaveBeenLastCalledWith(
    model,
    '1flowbase:block-source:file:///block.tsx',
    [
      expect.objectContaining({
        code: 'import_denied',
        message: "Import source 'dayjs' is not allowed.",
        severity: 8,
        startLineNumber: 1,
        startColumn: 1
      })
    ]
  );

  view.rerender(<BlockSourceEditor {...props} diagnostics={[]} />);
  await waitFor(() =>
    expect(setModelMarkers).toHaveBeenLastCalledWith(
      model,
      '1flowbase:block-source:file:///block.tsx',
      []
    )
  );
});

test('loads current source declarations without blocking editing, then hydrates new-import completion', async () => {
  const current = { source: 'current', filePath: 'file:///current.d.ts', content: 'export const current: string;' };
  const next = { source: 'new-import', filePath: 'file:///next.d.ts', content: 'export const next: string;' };
  const loader = { loadForSource: vi.fn(async () => [current]), loadAll: vi.fn(async () => [current, next]) };
  const dispose = vi.fn();
  const addExtraLib = vi.fn(() => ({ dispose }));
  const registerCompletionItemProvider = vi.fn((_language: string, _provider: unknown) => ({ dispose: vi.fn() }));
  const model = { uri: { toString: () => 'file:///source.tsx' } };
  const editor = { getModel: () => model, hasTextFocus: () => true, trigger: vi.fn() };
  const monaco = {
    MarkerSeverity: { Error: 8 }, editor: { setModelMarkers: vi.fn() },
    languages: { registerCompletionItemProvider, typescript: {
      typescriptDefaults: { addExtraLib, setCompilerOptions: vi.fn() }
    } }
  };
  const view = render(<BlockSourceEditor ariaLabel="source" path="file:///source.tsx" value="import 'current'" onChange={vi.fn()} declarationLoader={loader} />);
  await waitFor(() => expect(editorHarness.props).not.toBeNull());
  act(() => editorHarness.props?.onMount?.(editor, monaco));
  await waitFor(() => expect(addExtraLib).toHaveBeenCalledWith(current.content, current.filePath));
  expect(loader.loadAll).not.toHaveBeenCalled();
  const provider = registerCompletionItemProvider.mock.calls[0]?.[1] as unknown as {
    provideCompletionItems(model: unknown, position: unknown, context: unknown, token: unknown): Promise<unknown>;
  };
  await act(async () => { await provider.provideCompletionItems(model, {}, {}, { isCancellationRequested: false }); });
  expect(addExtraLib).toHaveBeenCalledWith(next.content, next.filePath);
  await waitFor(() => expect(editor.trigger).toHaveBeenCalledWith('native-declarations', 'editor.action.triggerSuggest', {}));
  view.unmount();
  expect(dispose).toHaveBeenCalledTimes(2);
});
