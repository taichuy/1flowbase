import type { editor } from 'monaco-editor';
import { Suspense, lazy, useMemo } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';

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
  onChange
}: {
  label: string;
  language?: 'javascript' | 'sql';
  value: unknown;
  onChange: (value: string) => void;
}) {
  const source = typeof value === 'string' ? value : '';
  const options = useMemo(
    () => ({
      ...CODE_EDITOR_OPTIONS,
      ariaLabel: label
    }),
    [label]
  );

  return (
    <div className="agent-flow-code-source-field">
      <Suspense fallback={<CodeSourceEditorFallback language={language} />}>
        <MonacoEditor
          defaultLanguage={language}
          height="260px"
          language={language}
          options={options}
          theme="vs"
          value={source}
          onChange={(nextValue) => onChange(nextValue ?? '')}
        />
      </Suspense>
    </div>
  );
}
