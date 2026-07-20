import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import {
  applyStoredWidth,
  DEFAULT_MIN_HEIGHT,
  DEFAULT_MIN_WIDTH,
  writeStoredWidth,
  type FloatingWindowRect
} from './floating-window-geometry';

export type ApplicationLogsFloatingWindowProps = {
  active: boolean;
  children: ReactNode;
  className?: string;
  minHeight?: number;
  minWidth?: number;
  testId: string;
  title: string;
  initialRect: () => FloatingWindowRect;
  onActivate: () => void;
  rect?: FloatingWindowRect;
  onRectChange?: (rect: FloatingWindowRect) => void;
};

export function ApplicationLogsFloatingWindow({
  active,
  children,
  className,
  minHeight = DEFAULT_MIN_HEIGHT,
  minWidth = DEFAULT_MIN_WIDTH,
  testId,
  title,
  initialRect,
  onActivate,
  rect,
  onRectChange
}: ApplicationLogsFloatingWindowProps) {
  const { t } = useTranslation('applications');
  return (
    <WindowWorkspaceWindow
      active={active}
      bodyClassName="application-logs-floating-window__body"
      className={['application-logs-floating-window', className]
        .filter(Boolean)
        .join(' ')}
      dragHandleSelector=".agent-flow-editor__dock-panel-header, .application-run-detail__header"
      initialRect={() => applyStoredWidth(initialRect(), testId)}
      minHeight={minHeight}
      minWidth={minWidth}
      rect={rect}
      resizeEdges={['left', 'right', 'bottom']}
      resizeClassName={(edge) =>
        `application-logs-floating-window__resize application-logs-floating-window__resize--${edge}`
      }
      resizeLabel={(edge) => {
        if (edge === 'left') {
          return t('auto.adjust_width_from_left', { value1: title });
        }
        if (edge === 'right') {
          return t('auto.adjust_width_from_right', { value1: title });
        }
        return t('auto.adjust_height_downward', { value1: title });
      }}
      testId={testId}
      title={title}
      onActivate={onActivate}
      onInteractionEnd={(nextRect) => writeStoredWidth(testId, nextRect.width)}
      onRectChange={onRectChange}
    >
      {children}
    </WindowWorkspaceWindow>
  );
}
