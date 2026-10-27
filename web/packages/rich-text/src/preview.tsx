import { useEffect, useRef, useState } from 'react';
import { VditorFrame } from './frame';
import { loadFrameRuntime, sharedPreviewOptions } from './runtime-assets';

export interface VditorPreviewProps {
  readonly value: string;
  readonly className?: string;
  readonly ariaLabel?: string;
  readonly locale?: 'zh_Hans' | 'en_US';
  readonly theme?: 'classic' | 'dark';
}

/** Read and edit use the same Lute dialect, renderers, styles and local assets. */
export function VditorPreview({
  value,
  className,
  ariaLabel = 'Markdown',
  locale = 'zh_Hans',
  theme = 'classic'
}: VditorPreviewProps) {
  const [document, setDocument] = useState<Document | null>(null);
  const [height, setHeight] = useState(100);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);
  useEffect(() => {
    if (!document) return;
    const current = ++generation.current;
    let disposed = false;
    const content = document.createElement('div');
    content.id = 'preview';
    // Each render owns its own detached element until it is current.
    let observer: ResizeObserver | undefined;
    setFailed(false);
    void loadFrameRuntime(document)
      .then(async (Vditor) => {
        if (disposed) return;
        document.body.appendChild(content);
        await Vditor.preview(
          content,
          value,
          sharedPreviewOptions(locale, theme === 'dark', document)
        );
        if (disposed || current !== generation.current) return;
        content.querySelectorAll('a[href]').forEach((link) => {
          if (link.getAttribute('href')?.startsWith('#')) return;
          link.setAttribute('target', '_blank');
          link.setAttribute('rel', 'noopener noreferrer');
        });
        const resize = () =>
          setHeight(
            Math.max(1, Math.ceil(content.getBoundingClientRect().height))
          );
        const view = document.defaultView as Window & typeof globalThis;
        observer = new view.ResizeObserver(resize);
        observer.observe(content);
        resize();
      })
      .catch(() => {
        if (!disposed) setFailed(true);
      });
    return () => {
      disposed = true;
      observer?.disconnect();
      content.remove();
    };
  }, [document, value, locale, theme]);
  return (
    <div
      className={['oneflow-markdown-preview', className]
        .filter(Boolean)
        .join(' ')}
    >
      {failed && (
        <div role="alert">
          {locale === 'zh_Hans'
            ? '正文加载失败，请刷新重试。'
            : 'Could not load the document. Please refresh to retry.'}
        </div>
      )}
      <VditorFrame title={ariaLabel} height={height} onDocument={setDocument} />
    </div>
  );
}
