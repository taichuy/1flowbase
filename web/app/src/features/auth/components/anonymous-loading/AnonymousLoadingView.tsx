import React from 'react';

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
              <path d="M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 00-94.3-139.9 437.71 437.71 0 00-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z" />
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
