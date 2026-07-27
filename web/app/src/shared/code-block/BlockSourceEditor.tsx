import Editor, {
  type BeforeMount,
  type Monaco,
  type OnMount
} from '@monaco-editor/react';
import { useCallback, useEffect, useRef } from 'react';

import type { BlockSourceExtraLib } from './extra-lib';

export interface BlockSourceEditorProps {
  ariaLabel: string;
  diagnostics?: readonly BlockSourceEditorDiagnostic[];
  extraLibs?: readonly BlockSourceExtraLib[];
  height?: string | number;
  path: string;
  readOnly?: boolean;
  value: string;
  onChange: (value: string) => void;
  onMount?: OnMount;
}

export interface BlockSourceEditorDiagnostic {
  code?: string;
  message: string;
  sourceLocation?: {
    line: number;
    column: number;
    endLine?: number;
    endColumn?: number;
  };
}

export function BlockSourceEditor({
  ariaLabel,
  diagnostics = [],
  extraLibs = [],
  height = '100%',
  onChange,
  onMount,
  path,
  readOnly = false,
  value
}: BlockSourceEditorProps) {
  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const registeredMarkersRef = useRef<{
    monaco: Monaco;
    model: NonNullable<ReturnType<Parameters<OnMount>[0]['getModel']>>;
    owner: string;
  } | null>(null);
  const registeredExtraLibsRef = useRef<{
    source: readonly BlockSourceExtraLib[];
    disposables: Array<{ dispose: () => void }>;
  } | null>(null);
  const configureMonaco: BeforeMount = (monaco) => {
    monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
      allowNonTsExtensions: true,
      jsx: monaco.languages.typescript.JsxEmit.Preserve,
      moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
      target: monaco.languages.typescript.ScriptTarget.ES2022
    });
  };

  const registerExtraLibs = useCallback(
    (monaco: Monaco) => {
      if (registeredExtraLibsRef.current?.source === extraLibs) return;
      registeredExtraLibsRef.current?.disposables.forEach((disposable) =>
        disposable.dispose()
      );
      registeredExtraLibsRef.current = {
        source: extraLibs,
        disposables: extraLibs.map((extraLib) =>
          monaco.languages.typescript.typescriptDefaults.addExtraLib(
            extraLib.content,
            extraLib.filePath
          )
        )
      };
    },
    [extraLibs]
  );

  const registerDiagnostics = useCallback(
    (editor: Parameters<OnMount>[0], monaco: Monaco) => {
      if (
        typeof editor.getModel !== 'function' ||
        typeof monaco.editor?.setModelMarkers !== 'function'
      ) {
        return;
      }
      const model = editor.getModel();
      if (!model) return;
      const owner = `1flowbase:block-source:${path}`;
      const registered = registeredMarkersRef.current;
      if (
        registered &&
        (registered.model !== model || registered.owner !== owner)
      ) {
        registered.monaco.editor.setModelMarkers(
          registered.model,
          registered.owner,
          []
        );
      }
      monaco.editor.setModelMarkers(
        model,
        owner,
        diagnostics.map((diagnostic) => {
          const line = diagnostic.sourceLocation?.line ?? 1;
          const column = diagnostic.sourceLocation?.column ?? 1;
          return {
            severity: monaco.MarkerSeverity.Error,
            message: diagnostic.message,
            source: '1flowbase',
            ...(diagnostic.code ? { code: diagnostic.code } : {}),
            startLineNumber: line,
            startColumn: column,
            endLineNumber: diagnostic.sourceLocation?.endLine ?? line,
            endColumn:
              diagnostic.sourceLocation?.endColumn ?? Math.max(2, column + 1)
          };
        })
      );
      registeredMarkersRef.current = { monaco, model, owner };
    },
    [diagnostics, path]
  );

  useEffect(() => {
    const monaco = monacoRef.current;
    if (monaco) registerExtraLibs(monaco);
  }, [registerExtraLibs]);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (editor && monaco) registerDiagnostics(editor, monaco);
  }, [registerDiagnostics]);

  useEffect(
    () => () => {
      registeredExtraLibsRef.current?.disposables.forEach((disposable) =>
        disposable.dispose()
      );
      const registeredMarkers = registeredMarkersRef.current;
      if (registeredMarkers) {
        registeredMarkers.monaco.editor.setModelMarkers(
          registeredMarkers.model,
          registeredMarkers.owner,
          []
        );
      }
    },
    []
  );

  return (
    <div aria-label={ariaLabel} role="group" style={{ height }}>
      <Editor
        height="100%"
        language="typescript"
        path={path}
        value={value}
        beforeMount={configureMonaco}
        onMount={(editor, monaco) => {
          editorRef.current = editor;
          monacoRef.current = monaco;
          registerExtraLibs(monaco);
          registerDiagnostics(editor, monaco);
          onMount?.(editor, monaco);
        }}
        onChange={(nextValue) => onChange(nextValue ?? '')}
        options={{
          ariaLabel,
          automaticLayout: true,
          editContext: false,
          fontSize: 13,
          lineNumbersMinChars: 3,
          minimap: { enabled: false },
          padding: { top: 12, bottom: 12 },
          readOnly,
          scrollBeyondLastLine: false,
          tabSize: 2,
          wordWrap: 'on'
        }}
      />
    </div>
  );
}
