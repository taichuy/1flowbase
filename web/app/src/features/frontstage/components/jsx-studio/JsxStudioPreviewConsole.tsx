import type {
  NativeBlockContextApiCallObservation,
  NativeReactCompileDiagnostic,
  NativeReactRuntimeDiagnostic
} from '@1flowbase/page-runtime';
import { Typography } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';

export interface JsxStudioPreviewConsoleSnapshot {
  diagnostics: ReadonlyArray<
    NativeReactCompileDiagnostic | NativeReactRuntimeDiagnostic
  >;
  apiCalls: readonly NativeBlockContextApiCallObservation[];
}

const DEFAULT_PREVIEW_PERCENT = 65;
const MIN_PREVIEW_PERCENT = 20;
const MAX_PREVIEW_PERCENT = 80;
const KEYBOARD_STEP_PERCENT = 5;

export function JsxStudioPreviewConsole({
  preview,
  snapshot
}: {
  preview: ReactNode;
  snapshot: JsxStudioPreviewConsoleSnapshot;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const dragStartRef = useRef<{
    percent: number;
    pointerY: number;
  } | null>(null);
  const [previewPercent, setPreviewPercent] = useState(DEFAULT_PREVIEW_PERCENT);

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = dragStartRef.current;
      const height = containerRef.current?.getBoundingClientRect().height ?? 0;
      if (!dragStart || height <= 0) return;
      setPreviewPercent(
        clampPreviewPercent(
          dragStart.percent +
            ((event.clientY - dragStart.pointerY) / height) * 100
        )
      );
    };
    const handleMouseUp = () => {
      dragStartRef.current = null;
      document.body.classList.remove(
        'frontstage-js-block-preview-console--resizing'
      );
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.classList.remove(
        'frontstage-js-block-preview-console--resizing'
      );
    };
  }, []);

  return (
    <div
      ref={containerRef}
      className="frontstage-js-block-preview-console"
      data-testid="js-block-preview-console"
      style={{
        gridTemplateRows: `minmax(0, ${previewPercent}fr) 8px minmax(0, ${100 - previewPercent}fr)`
      }}
    >
      <section
        className="frontstage-js-block-preview-console__preview"
        data-testid="js-block-preview-pane"
      >
        {preview}
      </section>
      <div
        aria-label={i18nText('frontstage', 'auto.resize_preview_console')}
        aria-orientation="horizontal"
        aria-valuemax={MAX_PREVIEW_PERCENT}
        aria-valuemin={MIN_PREVIEW_PERCENT}
        aria-valuenow={previewPercent}
        className="frontstage-js-block-preview-console__resize-handle"
        role="separator"
        tabIndex={0}
        onKeyDown={(event) => {
          if (event.key === 'ArrowUp') {
            event.preventDefault();
            setPreviewPercent((current) =>
              clampPreviewPercent(current - KEYBOARD_STEP_PERCENT)
            );
          } else if (event.key === 'ArrowDown') {
            event.preventDefault();
            setPreviewPercent((current) =>
              clampPreviewPercent(current + KEYBOARD_STEP_PERCENT)
            );
          } else if (event.key === 'Home') {
            event.preventDefault();
            setPreviewPercent(MIN_PREVIEW_PERCENT);
          } else if (event.key === 'End') {
            event.preventDefault();
            setPreviewPercent(MAX_PREVIEW_PERCENT);
          }
        }}
        onMouseDown={(event) => {
          event.preventDefault();
          dragStartRef.current = {
            percent: previewPercent,
            pointerY: event.clientY
          };
          document.body.classList.add(
            'frontstage-js-block-preview-console--resizing'
          );
        }}
      />
      <section
        className="frontstage-js-block-preview-console__console"
        data-testid="js-block-console-pane"
      >
        <header className="frontstage-js-block-preview-console__console-header">
          <Typography.Text strong>
            {i18nText('frontstage', 'auto.console')}
          </Typography.Text>
        </header>
        <div className="frontstage-js-block-preview-console__console-content">
          <div
            aria-live="polite"
            className="frontstage-js-block-preview-console__log-list"
            role="log"
          >
            {snapshot.diagnostics.map((diagnostic, index) => (
              <ConsoleEntry
                key={`${diagnostic.phase}:${diagnostic.path}:${index}`}
                level="error"
                message={formatDiagnostic(diagnostic)}
              />
            ))}
            {snapshot.apiCalls.map((observation, index) => (
              <ConsoleEntry
                key={`${observation.callId}:${observation.status}:${index}`}
                level={
                  observation.status === 'failed'
                    ? 'error'
                    : observation.status === 'pending'
                      ? 'debug'
                      : 'info'
                }
                message={formatApiObservation(observation)}
              />
            ))}
            <div
              className="frontstage-js-block-preview-console__prompt"
              data-testid="js-block-console-prompt"
            >
              <span>&gt;</span>
              <span aria-hidden="true" />
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

function ConsoleEntry({
  level,
  message
}: {
  level: 'debug' | 'info' | 'error';
  message: string;
}) {
  return (
    <div
      className={[
        'frontstage-js-block-preview-console__log-entry',
        `frontstage-js-block-preview-console__log-entry--${level}`
      ].join(' ')}
    >
      <span
        className="frontstage-js-block-preview-console__log-gutter"
        data-testid={`js-block-console-gutter-${level}`}
        title={level}
      >
        {level === 'error' ? '×' : level === 'debug' ? '·' : '>'}
      </span>
      <div className="frontstage-js-block-preview-console__log-body">
        <Typography.Text code>{message}</Typography.Text>
      </div>
    </div>
  );
}

function clampPreviewPercent(value: number) {
  return Math.min(MAX_PREVIEW_PERCENT, Math.max(MIN_PREVIEW_PERCENT, value));
}

function formatDiagnostic(
  diagnostic: JsxStudioPreviewConsoleSnapshot['diagnostics'][number]
): string {
  const location = diagnostic.sourceLocation
    ? `:${diagnostic.sourceLocation.line}:${diagnostic.sourceLocation.column}`
    : '';
  return `[${diagnostic.phase}/${diagnostic.code}] ${diagnostic.path}${location} ${diagnostic.message}`;
}

function formatApiObservation(
  observation: NativeBlockContextApiCallObservation
): string {
  const duration =
    observation.status === 'pending'
      ? ''
      : ` ${Math.round(observation.durationMs)}ms`;
  const error = observation.error ? ` ${observation.error}` : '';
  return `[api/${observation.status}] ${observation.method} ${observation.path}${duration}${error}`;
}
