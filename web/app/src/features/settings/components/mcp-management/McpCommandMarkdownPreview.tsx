import { useEffect, useRef } from 'react';
import Vditor from 'vditor';
import 'vditor/dist/index.css';

export function McpCommandMarkdownPreview({
  content,
  ariaLabel
}: {
  content: string;
  ariaLabel: string;
}) {
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
      aria-label={ariaLabel}
      className="mcp-client-command-preview vditor-reset"
      role="region"
    />
  );
}
