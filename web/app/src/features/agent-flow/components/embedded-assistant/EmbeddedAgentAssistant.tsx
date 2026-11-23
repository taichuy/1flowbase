import { Menu, Tooltip } from 'antd';
import { lazy, Suspense, useLayoutEffect, useState } from 'react';
import type { ConsoleAssistantClientTools } from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/window-workspace-context';
import {
  ASSISTANT_WINDOW_ID,
  initialAssistantWindowRect
} from './assistant-window-geometry';
import './embedded-assistant.css';

const EmbeddedAgentAssistantWindowHost = lazy(() =>
  import('./EmbeddedAgentAssistantWindowHost').then((module) => ({
    default: module.EmbeddedAgentAssistantWindowHost
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
  const { close, open: openWindow } = useWindowWorkspace();

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
        <Suspense fallback={null}>
          <EmbeddedAgentAssistantWindowHost
            clientTools={clientTools}
            open={open}
            pageKey={pageKey}
            label={label}
            onClose={() => setOpen(false)}
          />
        </Suspense>
      ) : null}
    </>
  );
}
