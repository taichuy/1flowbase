import React from 'react';

const LOADING_COLOR = '#1677ff';
const LOADING_LABEL = 'thinking';

export interface LoadingStateProps {
  fullscreen?: boolean;
  compact?: boolean;
  className?: string;
}

// The browser supplies leaf imports; the build renderer supplies the same Ant
// components from its Node entrypoint. Only one loading layout owns the markup.
export function createLoadingState({
  Spin,
  Skeleton,
  LoadingOutlined
}: Pick<typeof import('antd'), 'Spin' | 'Skeleton'> &
  Pick<typeof import('@ant-design/icons'), 'LoadingOutlined'>) {
  const indicator = (
    <LoadingOutlined spin style={{ color: LOADING_COLOR, fontSize: 48 }} />
  );

  return function LoadingState({
    fullscreen = false,
    compact = false,
    className
  }: LoadingStateProps) {
    const classNames = [
      'loading-state',
      fullscreen ? 'loading-state--fullscreen' : null,
      compact ? 'loading-state--compact' : null,
      className
    ]
      .filter(Boolean)
      .join(' ');

    return (
      <div
        className={classNames}
        role="status"
        aria-live="polite"
        aria-label={LOADING_LABEL}
      >
        <Spin
          spinning
          description={LOADING_LABEL}
          indicator={indicator}
          size={compact ? 'medium' : 'large'}
          styles={{
            description: { color: LOADING_COLOR },
            indicator: { color: LOADING_COLOR }
          }}
        >
          <div className="loading-state__surface" aria-hidden="true">
            <Skeleton
              active
              title={{ width: compact ? '38%' : '28%' }}
              paragraph={{
                rows: compact ? 3 : 5,
                width: ['92%', '84%', '76%', '68%', '52%']
              }}
            />
          </div>
        </Spin>
      </div>
    );
  };
}
