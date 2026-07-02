import { Skeleton, Spin } from 'antd';

import './loading-state.css';

export interface LoadingStateProps {
  fullscreen?: boolean;
  compact?: boolean;
  className?: string;
}

export function LoadingState({
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
    <div className={classNames} role="status" aria-live="polite" aria-label="thinking">
      <Spin spinning tip="thinking" size={compact ? 'default' : 'large'}>
        <div className="loading-state__surface" aria-hidden="true">
          <Skeleton
            active
            title={{ width: compact ? '38%' : '28%' }}
            paragraph={{ rows: compact ? 3 : 5, width: ['92%', '84%', '76%', '68%', '52%'] }}
          />
        </div>
      </Spin>
    </div>
  );
}
