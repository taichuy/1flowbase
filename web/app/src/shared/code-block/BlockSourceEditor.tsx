import Editor, {
  type BeforeMount,
  type OnMount
} from '@monaco-editor/react';
import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';

export interface BlockSourceEditorProps {
  ariaLabel: string;
  extraLibs?: readonly FrontendBlockMonacoExtraLib[];
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
  const configureMonaco: BeforeMount = (monaco) => {
    monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
      allowNonTsExtensions: true,
      jsx: monaco.languages.typescript.JsxEmit.Preserve,
      moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
      target: monaco.languages.typescript.ScriptTarget.ES2022
    });
    extraLibs.forEach((extraLib) => {
      monaco.languages.typescript.typescriptDefaults.addExtraLib(
        extraLib.content,
        extraLib.filePath
      );
    });
  };

  return (
    <div aria-label={ariaLabel} role="group" style={{ height }}>
      <Editor
        height="100%"
        language="typescript"
        path={path}
        value={value}
        beforeMount={configureMonaco}
        onMount={onMount}
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
