import { Button } from 'antd';
import { useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { EmbeddedAgentAssistantPreview } from './EmbeddedAgentAssistantPreview';

export function EmbeddedAgentAssistant() {
  const [open, setOpen] = useState(false);
  const [previewMounted, setPreviewMounted] = useState(false);

  return (
    <>
      <Button
        aria-label={i18nText('appShell', 'auto.assistant')}
        aria-pressed={open}
        className={
          open
            ? 'embedded-agent-assistant-trigger embedded-agent-assistant-trigger--active'
            : 'embedded-agent-assistant-trigger'
        }
        type="text"
        onClick={() => {
          if (open) {
            setOpen(false);
            return;
          }
          setPreviewMounted(true);
          setOpen(true);
        }}
      >
        AI
      </Button>
      {previewMounted ? (
        <EmbeddedAgentAssistantPreview
          open={open}
          onClose={() => setOpen(false)}
        />
      ) : null}
    </>
  );
}
