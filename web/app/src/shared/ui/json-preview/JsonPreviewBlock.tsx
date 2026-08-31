import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import FullscreenOutlined from '@ant-design/icons/es/icons/FullscreenOutlined';
import { App, Button, Modal, Tooltip } from 'antd';
import type { ReactNode } from 'react';
import { Suspense, lazy, useMemo, useState } from 'react';

import { useClipboardCopy } from '../clipboard/use-clipboard-copy';
import './json-preview-block.css';
import { i18nText } from '../../i18n/text';

const MonacoEditor = lazy(() =>
  import('../../code-block/monaco-runtime').then(({ loadMonacoEditorModule }) =>
    loadMonacoEditorModule()
  )
);

const JSON_PREVIEW_MODAL_Z_INDEX = 1060;

const EDITOR_OPTIONS = {
  readOnly: true,
  domReadOnly: true,
  minimap: { enabled: false },
  scrollBeyondLastLine: false,
  wordWrap: 'on' as const,
  lineNumbersMinChars: 2,
  fontSize: 12,
  lineHeight: 18,
  folding: true,
  renderLineHighlight: 'none' as const,
  overviewRulerBorder: false,
  automaticLayout: true,
  padding: {
    top: 8,
    bottom: 8
  },
  scrollbar: {
    verticalScrollbarSize: 14,
    horizontalScrollbarSize: 12
  }
};

function formatJsonPreview(value: unknown) {
  const formatted = JSON.stringify(value, null, 2);

  return typeof formatted === 'string' ? formatted : String(formatted);
}

function JsonEditorFallback({ minHeight }: { minHeight: string }) {
  return (
    <div className="json-preview-block__loading" style={{ minHeight }}>
      {i18nText('sharedUi', 'auto.loading_json_viewer')}
    </div>
  );
}

function JsonEditor({ height, value }: { height: string; value: string }) {
  return (
    <Suspense fallback={<JsonEditorFallback minHeight={height} />}>
      <MonacoEditor
        defaultLanguage="json"
        height={height}
        options={EDITOR_OPTIONS}
        theme="vs"
        value={value}
      />
    </Suspense>
  );
}

export function JsonPreviewBlock({
  title,
  value,
  actions,
  className,
  collapsible = true,
  copyAriaLabel,
  copyFailureMessage = i18nText('sharedUi', 'auto.copy_failed'),
  copySuccessMessage = i18nText('sharedUi', 'auto.copied'),
  defaultCollapsed = false,
  displayTitle = title,
  fullscreenAriaLabel,
  height = '220px',
  rawText
}: {
  title: string;
  value: unknown;
  actions?: ReactNode;
  className?: string;
  collapsible?: boolean;
  copyAriaLabel?: string;
  copyFailureMessage?: string;
  copySuccessMessage?: string;
  defaultCollapsed?: boolean;
  displayTitle?: string;
  fullscreenAriaLabel?: string;
  height?: string;
  rawText?: string;
}) {
  const { message } = App.useApp();
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  const [expanded, setExpanded] = useState(false);
  const { copied, copy } = useClipboardCopy();
  const formattedValue = useMemo(
    () => rawText ?? formatJsonPreview(value),
    [rawText, value]
  );
  const isCollapsed = collapsible ? collapsed : false;

  const handleCopy = async () => {
    try {
      await copy(formattedValue);
      message.success(copySuccessMessage);
    } catch {
      message.error(copyFailureMessage);
    }
  };

  return (
    <section
      className={['json-preview-block', className].filter(Boolean).join(' ')}
    >
      <pre aria-label={`${title} JSON`} className="json-preview-block__a11y">
        {formattedValue}
      </pre>
      <div className="json-preview-block__header">
        <button
          aria-label={title}
          aria-expanded={collapsible ? !isCollapsed : undefined}
          className="json-preview-block__toggle"
          onClick={
            collapsible ? () => setCollapsed((current) => !current) : undefined
          }
          type="button"
        >
          {collapsible ? (
            <DownOutlined className="json-preview-block__toggle-icon" />
          ) : null}
          {displayTitle ? (
            <span className="json-preview-block__title">{displayTitle}</span>
          ) : null}
        </button>
        <div className="json-preview-block__actions">
          {actions}
          <Tooltip title={i18nText('sharedUi', 'auto.copy_json')}>
            <Button
              aria-label={
                copyAriaLabel ??
                i18nText('sharedUi', 'auto.copy_named_json', { value1: title })
              }
              icon={copied ? <CheckOutlined /> : <CopyOutlined />}
              onClick={handleCopy}
              size="small"
              type="text"
            />
          </Tooltip>
          <Tooltip title={i18nText('sharedUi', 'auto.enlarge_view')}>
            <Button
              aria-label={
                fullscreenAriaLabel ??
                i18nText('sharedUi', 'auto.zoom_view_named_json', {
                  value1: title
                })
              }
              disabled={isCollapsed}
              icon={<FullscreenOutlined />}
              onClick={() => setExpanded(true)}
              size="small"
              type="text"
            />
          </Tooltip>
        </div>
      </div>
      {!isCollapsed ? (
        <div className="json-preview-block__editor">
          <JsonEditor height={height} value={formattedValue} />
        </div>
      ) : null}
      <Modal
        centered
        className="json-preview-block-modal"
        footer={null}
        onCancel={() => setExpanded(false)}
        open={expanded}
        title={`${title} JSON`}
        width="min(960px, calc(100vw - 48px))"
        zIndex={JSON_PREVIEW_MODAL_Z_INDEX}
      >
        <div className="json-preview-block-modal__editor">
          <JsonEditor height="70vh" value={formattedValue} />
        </div>
      </Modal>
    </section>
  );
}
