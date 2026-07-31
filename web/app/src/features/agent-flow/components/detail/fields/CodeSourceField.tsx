import type { editor, IRange } from 'monaco-editor';
import { Button, Tooltip } from 'antd';
import { Suspense, lazy, useCallback, useEffect, useMemo, useRef } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';
import type { FlowSelectorOption } from '../../../lib/selector-options';
import { createTemplateSelectorToken } from '../../../lib/template-binding';

const MonacoEditor = lazy(() => import('@monaco-editor/react'));

const CODE_EDITOR_OPTIONS = {
  automaticLayout: true,
  minimap: { enabled: false },
  fontSize: 13,
  lineHeight: 20,
  lineNumbersMinChars: 3,
  scrollBeyondLastLine: false,
  tabSize: 2,
  wordWrap: 'on',
  padding: {
    top: 12,
    bottom: 12
  },
  scrollbar: {
    verticalScrollbarSize: 8,
    horizontalScrollbarSize: 8
  }
} satisfies editor.IStandaloneEditorConstructionOptions;

interface SqlVariableCompletionContext {
  query: string;
  range: IRange;
}

function getSqlVariableCompletionContext(
  line: string,
  position: { lineNumber: number; column: number }
): SqlVariableCompletionContext | null {
  const cursorOffset = position.column - 1;
  const linePrefix = line.slice(0, cursorOffset);
  const latestBraceOffset = linePrefix.lastIndexOf('{');

  if (latestBraceOffset < 0) {
    return null;
  }

  const triggerOffset =
    latestBraceOffset > 0 && linePrefix[latestBraceOffset - 1] === '{'
      ? latestBraceOffset - 1
      : latestBraceOffset;
  const query = linePrefix.slice(latestBraceOffset + 1);

  if (/[\s{}]/.test(query)) {
    return null;
  }

  let closingBraceCount = 0;

  while (
    closingBraceCount < 2 &&
    line[cursorOffset + closingBraceCount] === '}'
  ) {
    closingBraceCount += 1;
  }

  return {
    query,
    range: {
      startLineNumber: position.lineNumber,
      endLineNumber: position.lineNumber,
      startColumn: triggerOffset + 1,
      endColumn: position.column + closingBraceCount
    }
  };
}

function getSqlVariableMatchRank(
  option: FlowSelectorOption,
  normalizedQuery: string
) {
  if (!normalizedQuery) {
    return 0;
  }

  const primaryCandidates = [option.value.join('.'), option.displayLabel].map(
    (candidate) => candidate.toLowerCase()
  );
  const secondaryCandidates = [
    option.nodeLabel,
    option.outputLabel,
    option.outputKey
  ].map((candidate) => candidate.toLowerCase());

  if (
    primaryCandidates.some((candidate) => candidate.startsWith(normalizedQuery))
  ) {
    return 0;
  }

  if (
    secondaryCandidates.some((candidate) =>
      candidate.startsWith(normalizedQuery)
    )
  ) {
    return 1;
  }

  if (
    [...primaryCandidates, ...secondaryCandidates].some((candidate) =>
      candidate.includes(normalizedQuery)
    )
  ) {
    return 2;
  }

  return null;
}

function CodeSourceEditorFallback({
  language
}: {
  language: 'javascript' | 'sql';
}) {
  return (
    <div className="agent-flow-code-source-field__loading">
      {language === 'sql'
        ? i18nText('agentFlow', 'auto.loading_sql_editor')
        : i18nText('agentFlow', 'auto.loading_javascript_editor')}
    </div>
  );
}

export function CodeSourceField({
  label,
  language = 'javascript',
  value,
  variableOptions = [],
  onChange
}: {
  label: string;
  language?: 'javascript' | 'sql';
  value: unknown;
  variableOptions?: FlowSelectorOption[];
  onChange: (value: string) => void;
}) {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const completionProviderRef = useRef<{ dispose: () => void } | null>(null);
  const variableOptionsRef = useRef(variableOptions);
  const source = typeof value === 'string' ? value : '';
  variableOptionsRef.current = variableOptions;
  const options = useMemo(
    () => ({
      ...CODE_EDITOR_OPTIONS,
      ariaLabel: label
    }),
    [label]
  );

  useEffect(
    () => () => {
      completionProviderRef.current?.dispose();
      completionProviderRef.current = null;
    },
    []
  );

  const handleMount = useCallback(
    (
      mountedEditor: editor.IStandaloneCodeEditor,
      monaco: typeof import('monaco-editor')
    ) => {
      editorRef.current = mountedEditor;
      completionProviderRef.current?.dispose();

      if (language !== 'sql') {
        completionProviderRef.current = null;
        return;
      }

      const mountedModel = mountedEditor.getModel();
      completionProviderRef.current =
        monaco.languages.registerCompletionItemProvider('sql', {
          triggerCharacters: ['{'],
          provideCompletionItems(model, position) {
            if (model !== mountedModel) {
              return { suggestions: [] };
            }

            const completionContext = getSqlVariableCompletionContext(
              model.getLineContent(position.lineNumber),
              position
            );
            const range = completionContext?.range ?? {
              startLineNumber: position.lineNumber,
              endLineNumber: position.lineNumber,
              startColumn: position.column,
              endColumn: position.column
            };
            const normalizedQuery =
              completionContext?.query.trim().toLowerCase() ?? '';
            const matchedOptions = variableOptionsRef.current
              .flatMap((option, index) => {
                const rank = getSqlVariableMatchRank(option, normalizedQuery);

                return rank === null ? [] : [{ index, option, rank }];
              })
              .sort(
                (left, right) =>
                  left.rank - right.rank || left.index - right.index
              );

            return {
              incomplete: completionContext !== null,
              suggestions: matchedOptions.map(({ option, index, rank }) => {
                const token = createTemplateSelectorToken(option.value);

                return {
                  label: option.displayLabel,
                  kind: monaco.languages.CompletionItemKind.Variable,
                  insertText: token,
                  filterText: completionContext
                    ? `{${completionContext.query}`
                    : option.displayLabel,
                  range,
                  sortText: `${rank}${String(index).padStart(6, '0')}`
                };
              })
            };
          }
        });
    },
    [language]
  );

  const openVariableSuggestions = useCallback(() => {
    const mountedEditor = editorRef.current;

    if (!mountedEditor) {
      return;
    }

    mountedEditor.focus();
    mountedEditor.trigger(
      'sql-variable-toolbar',
      'editor.action.triggerSuggest',
      undefined
    );
  }, []);

  return (
    <div className="agent-flow-code-source-field nokey">
      {language === 'sql' ? (
        <div className="agent-flow-code-source-field__toolbar">
          <Tooltip title={i18nText('agentFlow', 'auto.insert_variable')}>
            <Button
              type="text"
              size="small"
              disabled={variableOptions.length === 0}
              aria-label={i18nText('agentFlow', 'auto.insert_variable')}
              onClick={openVariableSuggestions}
            >
              {'{x}'}
            </Button>
          </Tooltip>
        </div>
      ) : null}
      <Suspense fallback={<CodeSourceEditorFallback language={language} />}>
        <MonacoEditor
          defaultLanguage={language}
          height="260px"
          language={language}
          options={options}
          theme="vs"
          value={source}
          onMount={handleMount}
          onChange={(nextValue) => onChange(nextValue ?? '')}
        />
      </Suspense>
    </div>
  );
}
