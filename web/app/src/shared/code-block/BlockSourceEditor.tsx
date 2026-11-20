import { Button } from 'antd';
import type { BeforeMount, Monaco, OnMount } from '@monaco-editor/react';
import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useRef,
  useState
} from 'react';

import type {
  BlockSourceDeclarationLoader,
  BlockSourceExtraLib
} from './extra-lib';
import { createDeclarationSession } from './declarations/session';
import { i18nText } from '../i18n/text';
import { loadMonacoEditorModule } from './monaco-runtime';
import { LoadingState } from '../ui/loading-state/LoadingState';

const Editor = lazy(loadMonacoEditorModule);

export interface BlockSourceEditorProps {
  ariaLabel: string;
  diagnostics?: readonly BlockSourceEditorDiagnostic[];
  extraLibs?: readonly BlockSourceExtraLib[];
  declarationLoader?: BlockSourceDeclarationLoader;
  height?: string | number;
  path: string;
  readOnly?: boolean;
  value: string;
  onChange: (value: string, modelPath: string) => void;
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

const EMPTY_DIAGNOSTICS: readonly BlockSourceEditorDiagnostic[] = [];
const EMPTY_EXTRA_LIBS: readonly BlockSourceExtraLib[] = [];

const configureMonaco: BeforeMount = (monaco) => {
  monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
    allowNonTsExtensions: true,
    allowSyntheticDefaultImports: true,
    esModuleInterop: true,
    jsx: monaco.languages.typescript.JsxEmit.Preserve,
    moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
    target: monaco.languages.typescript.ScriptTarget.ES2022
  });
};

export function BlockSourceEditor({
  ariaLabel,
  diagnostics = EMPTY_DIAGNOSTICS,
  extraLibs = EMPTY_EXTRA_LIBS,
  declarationLoader,
  height = '100%',
  onChange,
  onMount,
  path,
  readOnly = false,
  value
}: BlockSourceEditorProps) {
  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const [ready, setReady] = useState(false);
  const registeredMarkersRef = useRef<{
    monaco: Monaco;
    model: NonNullable<ReturnType<Parameters<OnMount>[0]['getModel']>>;
    owner: string;
  } | null>(null);
  const declarationSessionRef = useRef<ReturnType<
    typeof createDeclarationSession
  > | null>(null);
  const [declarationError, setDeclarationError] = useState(false);
  const [declarationRetry, setDeclarationRetry] = useState(0);
  const registerExtraLibs = useCallback(
    (monaco: Monaco) => {
      declarationSessionRef.current ??= createDeclarationSession(monaco);
      declarationSessionRef.current.register(extraLibs);
    },
    [extraLibs]
  );

  useEffect(() => {
    if (!declarationLoader) return;
    let cancelled = false;
    // Start alongside the Monaco import, not after Monaco finishes mounting.
    void declarationLoader.loadForSource(value).then(
      (files) => {
        if (cancelled) return;
        declarationSessionRef.current?.register(files);
        setDeclarationError(false);
      },
      () => {
        if (!cancelled) setDeclarationError(true);
      }
    );
    return () => {
      cancelled = true;
    };
  }, [declarationLoader, value, ready, declarationRetry]);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    const session = declarationSessionRef.current;
    if (!ready || !declarationLoader || !editor || !monaco || !session) return;
    let disposed = false;
    let retriggered = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const provider = monaco.languages.registerCompletionItemProvider(
      'typescript',
      {
        triggerCharacters: ['"', "'", '/'],
        async provideCompletionItems(model, _position, _context, token) {
          if (model !== editor.getModel()) return { suggestions: [] };
          try {
            const changed = await session.loadAll(declarationLoader);
            if (!disposed) setDeclarationError(false);
            // Built-in TS suggestions may have started before addExtraLib. Once
            // the complete import inventory is installed, run that provider again.
            if (
              changed &&
              !retriggered &&
              !disposed &&
              !token.isCancellationRequested &&
              model === editor.getModel()
            ) {
              retriggered = true;
              timer = setTimeout(() => {
                if (
                  !disposed &&
                  model === editor.getModel() &&
                  editor.hasTextFocus()
                ) {
                  editor.trigger(
                    'native-declarations',
                    'editor.action.triggerSuggest',
                    {}
                  );
                }
              }, 0);
            }
          } catch {
            if (!disposed) setDeclarationError(true);
          }
          return { suggestions: [] };
        }
      }
    );
    return () => {
      disposed = true;
      clearTimeout(timer);
      provider.dispose();
    };
  }, [declarationLoader, ready, declarationRetry]);

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
      declarationSessionRef.current?.dispose();
      declarationSessionRef.current = null;
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
    <div
      aria-busy={!ready}
      aria-label={ariaLabel}
      role="group"
      style={{ height }}
    >
      {declarationError ? (
        <Button
          size="small"
          onClick={() => setDeclarationRetry((attempt) => attempt + 1)}
        >
          {i18nText('frontstage', 'auto.code_load_or_save_failed')} ·{' '}
          {i18nText('frontstage', 'auto.retry')}
        </Button>
      ) : null}
      <Suspense fallback={<LoadingState compact />}>
        <Editor
          height="100%"
          language="typescript"
          path={path}
          value={value}
          beforeMount={configureMonaco}
          onMount={(editor, monaco) => {
            editorRef.current = editor;
            monacoRef.current = monaco;
            setReady(true);
            registerExtraLibs(monaco);
            registerDiagnostics(editor, monaco);
            onMount?.(editor, monaco);
          }}
          onChange={(nextValue) => {
            const modelPath = editorRef.current?.getModel()?.uri.toString();
            if (modelPath) onChange(nextValue ?? '', modelPath);
          }}
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
      </Suspense>
    </div>
  );
}
