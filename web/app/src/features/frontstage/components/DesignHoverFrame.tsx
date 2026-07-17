import { HolderOutlined, SettingOutlined } from '@ant-design/icons';
import { Button, Space, Tooltip } from 'antd';
import type { CSSProperties, FC, PointerEvent, ReactNode } from 'react';
import { useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';

export interface DesignHoverFrameProps {
  /** Whether design mode is active. When false, children render untouched. */
  active: boolean;
  /** Localized label for the configure action, e.g. "配置页面". */
  configureLabel: string;
  /** Localized label for the drag handle, e.g. "拖拽排序". */
  dragLabel?: string;
  onConfigure: () => void;
  /** Optional extra toolbar actions rendered before the configure button. */
  extraActions?: ReactNode;
  /** Enables the drag handle and forwards pointer-down to the caller. */
  draggable?: boolean;
  onDragHandlePointerDown?: (event: PointerEvent<HTMLElement>) => void;
  style?: CSSProperties;
  className?: string;
  children: ReactNode;
}

const frameBaseStyle: CSSProperties = {
  position: 'relative',
  borderRadius: 8,
  border: '1px solid transparent',
  transition: 'box-shadow 0.15s ease, background 0.15s ease, border-color 0.15s ease'
};

const toolbarStyle: CSSProperties = {
  position: 'absolute',
  top: 4,
  right: 4,
  zIndex: 5,
  background: '#fff',
  border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.toolbarBorder}`,
  borderRadius: 8,
  boxShadow: FRONTSTAGE_DESIGN_BLUE.toolbarShadow,
  padding: 2,
  transition: 'opacity 0.15s ease'
};

/**
 * Shared design-mode affordance: a hover halo plus a floating toolbar with a
 * configure action and an optional drag handle. Used across the page-title,
 * page-tab, and (future) region levels so the three tiers share one interaction
 * language. Blocks keep their own BlockHoverToolbar.
 */
export const DesignHoverFrame: FC<DesignHoverFrameProps> = ({
  active,
  configureLabel,
  dragLabel,
  onConfigure,
  extraActions,
  draggable = false,
  onDragHandlePointerDown,
  style,
  className,
  children
}) => {
  const [isHovered, setIsHovered] = useState(false);

  if (!active) {
    return <>{children}</>;
  }

  const frameStyle: CSSProperties = {
    ...frameBaseStyle,
    borderColor: isHovered
      ? FRONTSTAGE_DESIGN_BLUE.borderHover
      : FRONTSTAGE_DESIGN_BLUE.borderIdle,
    ...(isHovered
      ? {
          background: FRONTSTAGE_DESIGN_BLUE.bgHover,
          boxShadow: FRONTSTAGE_DESIGN_BLUE.halo
        }
      : {}),
    ...style
  };

  const configureButtonStyle: CSSProperties = {
    color: FRONTSTAGE_DESIGN_BLUE.primary
  };

  return (
    <div
      className={className}
      style={frameStyle}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div style={{ ...toolbarStyle, opacity: isHovered ? 1 : 0, pointerEvents: isHovered ? 'auto' : 'none' }}>
        <Space size={2}>
          {extraActions}
          {draggable ? (
            <Tooltip title={dragLabel ?? i18nText('frontstage', 'design.drag_handle')}>
              <Button
                size="small"
                type="text"
                aria-label={dragLabel ?? i18nText('frontstage', 'design.drag_handle')}
                icon={<HolderOutlined />}
                style={{ cursor: 'grab', touchAction: 'none' }}
                onPointerDown={onDragHandlePointerDown}
              />
            </Tooltip>
          ) : null}
          <Tooltip title={configureLabel}>
            <Button
              size="small"
              type="text"
              aria-label={configureLabel}
              icon={<SettingOutlined />}
              style={configureButtonStyle}
              onClick={(event) => {
                event.stopPropagation();
                onConfigure();
              }}
            />
          </Tooltip>
        </Space>
      </div>
      {children}
    </div>
  );
};
