import HolderOutlined from '@ant-design/icons/es/icons/HolderOutlined';
import { Typography } from 'antd';
import type { MouseEventHandler, ReactNode } from 'react';
import { i18nText } from '../../../../shared/i18n/text';

interface NodeConfigFieldContainerClassNames {
  root?: string;
  frame?: string;
  toolbar?: string;
  label?: string;
  actions?: string;
}

interface NodeConfigFieldContainerProps {
  label: string;
  ariaLabel?: string;
  labelContent?: ReactNode;
  headerActions?: ReactNode;
  draggable?: boolean;
  dragLabel?: string;
  classNames?: NodeConfigFieldContainerClassNames;
  children: ReactNode;
  onDragEnd?: () => void;
  onDragStart?: () => void;
  onFrameMouseDown?: MouseEventHandler<HTMLDivElement>;
}

function joinClassNames(...classNames: Array<string | undefined>) {
  return classNames.filter(Boolean).join(' ');
}

export function NodeConfigFieldContainer({
  label,
  ariaLabel,
  labelContent,
  headerActions,
  draggable = false,
  dragLabel,
  classNames,
  children,
  onDragEnd,
  onDragStart,
  onFrameMouseDown
}: NodeConfigFieldContainerProps) {
  return (
    <div
      className={joinClassNames(
        'agent-flow-node-config-field',
        classNames?.root
      )}
    >
      <div
        className={joinClassNames(
          'agent-flow-node-config-field__frame',
          classNames?.frame
        )}
        onMouseDown={onFrameMouseDown}
      >
        <div
          className={joinClassNames(
            'agent-flow-node-config-field__toolbar',
            classNames?.toolbar
          )}
        >
          <div className="agent-flow-node-config-field__heading">
            {draggable ? (
              <button
                aria-label={
                  dragLabel ??
                  i18nText('agentFlow', 'auto.drag_drop_sort', {
                    value1: ariaLabel ?? label
                  })
                }
                className="agent-flow-node-config-field__drag-handle"
                draggable
                onDragEnd={onDragEnd}
                onDragStart={onDragStart}
                type="button"
              >
                <HolderOutlined />
              </button>
            ) : null}
            {labelContent ?? (
              <Typography.Text
                strong
                className={joinClassNames(
                  'agent-flow-node-config-field__label',
                  classNames?.label
                )}
              >
                {label}
              </Typography.Text>
            )}
          </div>
          <div
            className={joinClassNames(
              'agent-flow-node-config-field__actions',
              classNames?.actions
            )}
          >
            {headerActions}
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}
