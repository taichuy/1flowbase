import { act, render, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { BlockSourceEditor } from '../BlockSourceEditor';

interface EditorHarnessProps {
  beforeMount?: (monaco: unknown) => void;
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
