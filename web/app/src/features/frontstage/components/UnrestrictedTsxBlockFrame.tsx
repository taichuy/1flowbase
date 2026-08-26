import { createUnrestrictedTsxBlockSrcdoc, transformUnrestrictedTsxBlockSource } from '@1flowbase/page-runtime';
import { Alert } from 'antd';
import type { CSSProperties } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

const MINIMUM_FRAME_HEIGHT = 24;
const INITIAL_FRAME_HEIGHT = 160;

export function UnrestrictedTsxBlockFrame({
  blockId,
  source,
  style
}: {
  blockId: string;
  source: string;
  style: CSSProperties;
}) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const [height, setHeight] = useState(INITIAL_FRAME_HEIGHT);
  const transformed = useMemo(
    () => transformUnrestrictedTsxBlockSource(source),
    [source]
  );
  const srcDoc = useMemo(() => {
    if (!transformed.ok || typeof window === 'undefined') return '';
    return createUnrestrictedTsxBlockSrcdoc({
      moduleSource: transformed.moduleSource,
      baseUrl: window.location.href
    });
  }, [transformed]);

  useEffect(() => setHeight(INITIAL_FRAME_HEIGHT), [srcDoc]);
  useEffect(() => {
    const receiveHeight = (event: MessageEvent<unknown>) => {
      if (event.source !== iframeRef.current?.contentWindow) return;
      const payload = event.data;
      if (
        !payload ||
        typeof payload !== 'object' ||
        (payload as { type?: unknown }).type !==
          '1flowbase_unrestricted_tsx_height'
      ) {
        return;
      }
      const nextHeight = (payload as { height?: unknown }).height;
      if (!Number.isFinite(nextHeight)) return;
      setHeight(Math.max(MINIMUM_FRAME_HEIGHT, Math.ceil(nextHeight as number)));
    };
    window.addEventListener('message', receiveHeight);
    return () => window.removeEventListener('message', receiveHeight);
  }, []);

  if (!transformed.ok) {
    return (
      <div style={style}>
        <Alert
          type="error"
          showIcon
          title="TSX 编译失败"
          description={transformed.errors[0] ?? 'TSX Block 无法执行。'}
        />
      </div>
    );
  }

  return (
    <div style={style}>
      <iframe
        ref={iframeRef}
        title={`TSX Block ${blockId}`}
        sandbox="allow-scripts"
        srcDoc={srcDoc}
        data-testid={`frontstage-unrestricted-tsx-frame-${blockId}`}
        style={{
          display: 'block',
          width: '100%',
          height,
          border: 0
        }}
      />
    </div>
  );
}
