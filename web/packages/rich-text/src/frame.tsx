import type { CSSProperties } from 'react';

import { VDITOR_ASSET_BASE } from './asset-contract';

export function VditorFrame({
  title,
  height,
  onDocument
}: {
  title: string;
  height: CSSProperties['height'];
  onDocument(document: Document | null): void;
}) {
  return (
    <iframe
      title={title}
      src={`${VDITOR_ASSET_BASE}/frame.html`}
      allowFullScreen
      className="oneflow-vditor-frame"
      style={{ display: 'block', width: '100%', height, border: 0 }}
      onLoad={(event) => onDocument(event.currentTarget.contentDocument)}
    />
  );
}
