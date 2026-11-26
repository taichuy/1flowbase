import React from 'react';

export const AUTH_LOADING_ICON_PATH =
  'M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 00-94.3-139.9 437.71 437.71 0 00-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z';

export const AUTH_ERROR_ICON_PATH =
  'M512 64c247.4 0 448 200.6 448 448S759.4 960 512 960 64 759.4 64 512 264.6 64 512 64zm127.98 274.82h-.04l-.08.06L512 466.75 384.14 338.88c-.04-.05-.06-.06-.08-.06a.12.12 0 00-.07 0c-.03 0-.05.01-.09.05l-45.02 45.02a.2.2 0 00-.05.09.12.12 0 000 .07v.02a.27.27 0 00.06.06L466.75 512 338.88 639.86c-.05.04-.06.06-.06.08a.12.12 0 000 .07c0 .03.01.05.05.09l45.02 45.02a.2.2 0 00.09.05.12.12 0 00.07 0c.02 0 .04-.01.08-.05L512 557.25l127.86 127.87c.04.04.06.05.08.05a.12.12 0 00.07 0c.03 0 .05-.01.09-.05l45.02-45.02a.2.2 0 00.05-.09.12.12 0 000-.07v-.02a.27.27 0 00-.05-.06L557.25 512l127.87-127.86c.04-.04.05-.06.05-.08a.12.12 0 000-.07c0-.03-.01-.05-.05-.09l-45.02-45.02a.2.2 0 00-.09-.05.12.12 0 00-.07 0z';

export interface AnonymousLoadingStateProps {
  fullscreen?: boolean;
  compact?: boolean;
  className?: string;
  showSpinner?: boolean;
  showSkeleton?: boolean;
}

// Same geometry as LoadingState's emerald-theme Skeleton/Spin, without its
// runtime providers. The document renderer and anonymous React tree share this.
export function AnonymousLoadingState({
  fullscreen = false,
  compact = false,
  className,
  showSpinner = true,
  showSkeleton = true
}: AnonymousLoadingStateProps) {
  return (
    <div
      className={[
        'loading-state anonymous-loading',
        fullscreen && 'loading-state--fullscreen',
        compact && 'loading-state--compact',
        className
      ]
        .filter(Boolean)
        .join(' ')}
      role="status"
      aria-live="polite"
      aria-label="thinking"
    >
      <div
        className={`anonymous-loading__spin${showSpinner ? ' anonymous-loading__spin--active' : ''}`}
      >
        {showSpinner && (
          <div className="anonymous-loading__indicator" aria-hidden="true">
            <svg viewBox="0 0 1024 1024" focusable="false" fill="currentColor">
              <path d={AUTH_LOADING_ICON_PATH} />
            </svg>
            <span>thinking</span>
          </div>
        )}
        <div className="anonymous-loading__container" aria-hidden="true">
          <div className="loading-state__surface">
            {showSkeleton && (
              <div className="anonymous-loading__skeleton">
                <div className="anonymous-loading__section">
                  <h3 style={{ width: compact ? '38%' : '28%' }} />
                  <ul>
                    {['92%', '84%', '76%', '68%', '52%']
                      .slice(0, compact ? 3 : 5)
                      .map((width) => (
                        <li key={width} style={{ width }} />
                      ))}
                  </ul>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
