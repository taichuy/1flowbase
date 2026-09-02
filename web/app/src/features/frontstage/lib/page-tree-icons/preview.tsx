import type { CSSProperties } from 'react';
import { pageTreeIconPreviewHref } from 'virtual:1flowbase-page-tree-icon-previews';

type PageTreeIconPreviewProps = {
  name: string;
  className?: string;
  style?: CSSProperties;
};

function PageTreeIconPreview({
  name,
  className,
  style
}: PageTreeIconPreviewProps) {
  const href = pageTreeIconPreviewHref(name);
  if (!href) return null;
  return (
    <svg aria-hidden className={className} focusable="false" style={style}>
      <use href={href} />
    </svg>
  );
}

export { PageTreeIconPreview };
