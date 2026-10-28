import type { CSSProperties } from 'react';

import { VDITOR_ASSET_BASE } from './asset-contract';

export function VditorFrame({
  title,
  height,
  onDocument,
  loading = false,
  loadingLabel = '加载中…'
}: {
  title: string;
  height: CSSProperties['height'];
  onDocument(document: Document | null): void;
  loading?: boolean;
  loadingLabel?: string;
}) {
  return (
    <div aria-busy={loading} style={{ position: 'relative' }}>
      {loading && (
        <div
          role="status"
          style={{
            position: 'absolute',
            inset: 0,
            zIndex: 1,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 8,
            background: 'var(--ant-colorBgContainer, #fff)',
            color: 'var(--ant-colorTextSecondary, #57605d)'
          }}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
            <circle
              cx="12"
              cy="12"
              r="9"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              opacity=".2"
            />
            <path
              d="M12 3a9 9 0 0 1 9 9"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <animateTransform
                attributeName="transform"
                type="rotate"
                from="0 12 12"
                to="360 12 12"
                dur="1s"
                repeatCount="indefinite"
              />
            </path>
          </svg>
          {loadingLabel}
        </div>
      )}
      <iframe
        title={title}
        src={`${VDITOR_ASSET_BASE}/frame.html`}
        allowFullScreen
        className="oneflow-vditor-frame"
        style={{
          display: 'block',
          width: '100%',
          height,
          border: 0,
          visibility: loading ? 'hidden' : 'visible'
        }}
        onLoad={(event) => onDocument(event.currentTarget.contentDocument)}
      />
    </div>
  );
}
