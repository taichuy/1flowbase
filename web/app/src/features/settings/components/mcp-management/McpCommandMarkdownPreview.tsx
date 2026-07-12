import { useEffect, useRef } from 'react';
import Vditor from 'vditor';
import 'vditor/dist/index.css';

import { i18nText } from '../../../../shared/i18n/text';

export function McpCommandMarkdownPreview({ content }: { content: string }) {
  const previewRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const previewElement = previewRef.current;
    if (!previewElement) {
      return;
    }

    void Vditor.preview(previewElement, content, {
      mode: 'light',
      lang: 'zh_CN',
      anchor: 0,
      hljs: {
        enable: true,
        lineNumber: false,
        style: 'github'
      }
    });
  }, [content]);

  return (
    <div
      ref={previewRef}
      aria-label={i18nText(
        'settingsMcpManagement',
        'auto.command_preview'
      )}
      className="mcp-client-command-preview vditor-reset"
      role="region"
    />
  );
}
