import Editor, {
  type BeforeMount,
  type Monaco,
  type OnMount
} from '@monaco-editor/react';
import { useCallback, useEffect, useRef } from 'react';

import type { BlockSourceExtraLib } from './extra-lib';

export interface BlockSourceEditorProps {
  ariaLabel: string;
  extraLibs?: readonly BlockSourceExtraLib[];
  height?: string | number;
  path: string;
  readOnly?: boolean;
  value: string;
  onChange: (value: string) => void;
  onMount?: OnMount;
}

export function BlockSourceEditor({
  ariaLabel,
  extraLibs = [],
  height = '100%',
  onChange,
  onMount,
  path,
  readOnly = false,
  value
}: BlockSourceEditorProps) {
  const monacoRef = useRef<Monaco | null>(null);
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

  useEffect(() => {
    const monaco = monacoRef.current;
    if (monaco) registerExtraLibs(monaco);
  }, [registerExtraLibs]);

  useEffect(
    () => () => {
      registeredExtraLibsRef.current?.disposables.forEach((disposable) =>
        disposable.dispose()
      );
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
          monacoRef.current = monaco;
          registerExtraLibs(monaco);
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
