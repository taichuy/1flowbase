import { Menu, Tooltip } from 'antd';
import { useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { EmbeddedAgentAssistantPreview } from './EmbeddedAgentAssistantPreview';

export function EmbeddedAgentAssistant() {
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
        <EmbeddedAgentAssistantPreview
          open={open}
          onClose={() => setOpen(false)}
        />
      ) : null}
    </>
  );
}
