import { Menu, Tooltip } from 'antd';
import { lazy, Suspense, useState } from 'react';
import type { ConsoleAssistantClientTools } from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
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

  function toggleAssistant() {
    if (open) {
      setOpen(false);
      return;
    }
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
            open ? (
              <div
                aria-busy="true"
                className="embedded-agent-assistant-window-shell"
                data-testid="embedded-agent-assistant-window-shell"
              />
            ) : null
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
