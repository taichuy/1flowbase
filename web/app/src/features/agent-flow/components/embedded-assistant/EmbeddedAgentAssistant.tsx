import { Menu, Tooltip } from 'antd';
import { lazy, Suspense, useLayoutEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import type { ConsoleAssistantClientTools } from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import {
  ASSISTANT_WINDOW_ID,
  initialAssistantWindowRect
} from './assistant-window-geometry';
import './embedded-assistant.css';

const EmbeddedAgentAssistantPreview = lazy(() =>
  import('./EmbeddedAgentAssistantPreview').then((module) => ({
    default: module.EmbeddedAgentAssistantPreview
  }))
);

export function EmbeddedAgentAssistant({
  clientTools,
  pageKey = typeof window === 'undefined' ? '/' : window.location.pathname
}: {
  clientTools?: ConsoleAssistantClientTools;
  pageKey?: string;
}) {
  const [open, setOpen] = useState(false);
  const [previewMounted, setPreviewMounted] = useState(false);
  const label = i18nText('appShell', 'auto.assistant');
  const {
    activate,
    close,
    open: openWindow,
    setRect,
    state: windowWorkspaceState
  } = useWindowWorkspace();
  const windowEntry = windowWorkspaceState.windows.find(
    (entry) => entry.id === ASSISTANT_WINDOW_ID
  );

  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    return () => close(ASSISTANT_WINDOW_ID);
  }, [close, open]);

  function toggleAssistant() {
    if (open) {
      setOpen(false);
      return;
    }
    openWindow({
      id: ASSISTANT_WINDOW_ID,
      owner: 'embedded-agent-assistant',
      parent_id: null,
      rect: initialAssistantWindowRect(),
      dirty: false
    });
    setPreviewMounted(true);
    setOpen(true);
  }

  return (
    <>
      <Tooltip title={label}>
        <Menu
          className="app-shell-design-menu"
          disabledOverflow
          items={[
            {
              key: 'embedded-agent-assistant',
              className: open
                ? 'embedded-agent-assistant-trigger app-shell-design-mode-button ant-menu-item-selected'
                : 'embedded-agent-assistant-trigger app-shell-design-mode-button',
              label: (
                <span
                  aria-label={label}
                  aria-pressed={open}
                  className="app-shell-design-block"
                  role="button"
                >
                  AI
                </span>
              )
            }
          ]}
          mode="horizontal"
          selectable={false}
          selectedKeys={open ? ['embedded-agent-assistant'] : []}
          onClick={toggleAssistant}
        />
      </Tooltip>
      {previewMounted ? (
        <Suspense
          fallback={
            open && windowEntry
              ? createPortal(
                  <WindowWorkspaceWindow
                    active={
                      windowEntry.z_index ===
                      Math.max(
                        ...windowWorkspaceState.windows.map(
                          (entry) => entry.z_index
                        )
                      )
                    }
                    bodyClassName="embedded-agent-assistant-window-shell__body"
                    className="embedded-agent-assistant-window-shell"
                    dragHandleSelector="[data-assistant-loading-drag-handle]"
                    initialRect={() => windowEntry.rect}
                    minHeight={320}
                    minWidth={400}
                    rect={windowEntry.rect}
                    resizeEdges={[]}
                    resizeLabel={() => label}
                    testId="embedded-agent-assistant-window-shell"
                    title={label}
                    zIndex={1050 + windowEntry.z_index}
                    onActivate={() => activate(ASSISTANT_WINDOW_ID)}
                    onRectChange={(rect) =>
                      setRect(ASSISTANT_WINDOW_ID, rect)
                    }
                  >
                    <div aria-busy="true" />
                  </WindowWorkspaceWindow>,
                  document.body
                )
              : null
          }
        >
          <EmbeddedAgentAssistantPreview
            clientTools={clientTools}
            open={open}
            pageKey={pageKey}
            onClose={() => setOpen(false)}
          />
        </Suspense>
      ) : null}
    </>
  );
}
