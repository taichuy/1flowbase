import type { CSSProperties } from 'react';

import { getSeededPublicSystemMetadata } from '../../../../shared/api/system-metadata-seed';

export type WorkspaceShellSkeletonMode =
  | 'full'
  | 'below-topbar'
  | 'canvas-only';

const pill = (width: number | string, height = 32, background = '#16211d0c'): CSSProperties => ({
  width,
  height,
  borderRadius: 8,
  background
});

const col = (gap: number, extra?: CSSProperties): CSSProperties => ({
  display: 'flex',
  flexDirection: 'column',
  gap,
  ...extra
});

const row = (gap: number, extra?: CSSProperties): CSSProperties => ({
  display: 'flex',
  alignItems: 'center',
  gap,
  ...extra
});

const cardStyle = col(14, {
  height: 320,
  borderRadius: 8,
  background: '#fff',
  border: '1px solid #16211d0d',
  boxShadow: '0 14px 40px #2563eb0d',
  padding: 20
});

function CanvasCards() {
  return (
    <div style={col(12)} aria-hidden="true">
      {[0, 1].map((c) => (
        <div key={c} style={cardStyle}>
          {([['24%', 18], ['92%', 14], ['76%', 14]] as const).map(([w, h]) => (
            <div key={w} style={pill(w, h)} />
          ))}
          <div style={{ flex: 1, borderRadius: 8, background: '#16211d06' }} />
        </div>
      ))}
    </div>
  );
}

export function WorkspaceShellSkeleton({
  mode = 'full'
}: {
  mode?: WorkspaceShellSkeletonMode;
}) {
  const seeded = getSeededPublicSystemMetadata();
  const brandTitle = seeded?.system_name?.trim() || '1flowbase';
  const brandLogo = seeded?.logo_url?.trim() || '/icon.svg';

  return (
    <div
      data-testid="workspace-shell-skeleton"
      data-skeleton-mode={mode}
      role="status"
      aria-live="polite"
      aria-label="thinking"
      style={col(0, { width: '100%', minWidth: 0, minHeight: mode === 'full' ? '100vh' : undefined })}
    >
      {mode === 'full' ? (
        <header
          style={row(0, {
            height: 56,
            padding: '0 24px',
            justifyContent: 'space-between',
            background: '#f8fcfab8',
            borderBottom: '1px solid #00ab7314'
          })}
        >
          <div style={row(12)}>
            <img src={brandLogo} alt="" width={20} height={20} />
            <span style={{ fontSize: 17, fontWeight: 600, lineHeight: 1, color: '#16211d' }}>
              {brandTitle}
            </span>
          </div>
          <div style={row(14)} aria-hidden="true">
            {([[28, 22], [28, 22], [52, 24]] as const).map(([w, h], i) => (
              <span key={i} style={pill(w, h)} />
            ))}
          </div>
        </header>
      ) : null}
      {mode === 'canvas-only' ? (
        <CanvasCards />
      ) : (
        <div style={{ display: 'flex', gap: 16, flex: 1, minHeight: 'calc(100vh - 56px)' }} aria-hidden="true">
          <div style={col(10, { flex: '0 0 180px', padding: '24px 12px 0' })}>
            {['78%', '64%', '72%', '56%', '82%', '68%'].map((w, i) => (
              <div key={w} style={pill(w, 32, i ? undefined : '#00ab731a')} />
            ))}
          </div>
          <div style={{ flex: 1, minWidth: 0, padding: '24px 24px 24px 0' }}>
            <CanvasCards />
          </div>
        </div>
      )}
    </div>
  );
}
