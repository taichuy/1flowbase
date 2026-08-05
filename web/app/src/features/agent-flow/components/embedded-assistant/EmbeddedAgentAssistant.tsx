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
        type="text"
        onClick={() => {
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
