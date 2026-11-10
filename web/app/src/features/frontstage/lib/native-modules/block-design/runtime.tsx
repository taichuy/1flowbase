import {
  forwardRef,
  type ReactNode,
  type CSSProperties,
  type HTMLAttributes
} from 'react';
import {
  Button,
  Dropdown,
  Tooltip,
  type ButtonProps,
  type DropdownProps
} from 'antd';
import { FrontstageNodeActionButton } from '../../../components/FrontstageNodeActionButton';
import { MenuOutlined, SettingOutlined } from '@ant-design/icons';
import { FRONTSTAGE_DESIGN_BLUE } from '../../design-mode-theme';
import { BLOCK_BUTTON_METRICS } from './theme';
export { BLOCK_DESIGN_THEME } from './theme';
export { useRegisterBlockSettings } from '../../block-settings/registry';

/** Host-owned design controls, also available to compiled block code. */
export const BlockDesignButton = forwardRef<
  HTMLButtonElement | HTMLAnchorElement,
  Omit<ButtonProps, 'variant'> & {
    variant?: 'settings' | 'initializer';
  }
>(function BlockDesignButton(
  { variant = 'settings', className, style, children, icon, ...props },
  ref
) {
  const label =
    props['aria-label'] ??
    (typeof children === 'string' ? children : undefined);
  if (variant === 'initializer')
    return (
      <Button
        {...props}
        ref={ref}
        type="dashed"
        size="small"
        aria-label={label}
        icon={icon ?? <SettingOutlined />}
        className={`block-design-initializer ${className ?? ''}`}
        style={{
          color: FRONTSTAGE_DESIGN_BLUE.primary,
          borderColor: FRONTSTAGE_DESIGN_BLUE.dashed,
          height: BLOCK_BUTTON_METRICS.height,
          paddingInline: BLOCK_BUTTON_METRICS.paddingInline,
          columnGap: BLOCK_BUTTON_METRICS.gap,
          fontSize: BLOCK_BUTTON_METRICS.fontSize,
          fontWeight: BLOCK_BUTTON_METRICS.fontWeight,
          borderRadius: BLOCK_BUTTON_METRICS.borderRadius,
          ...style
        }}
      >
        {children ?? label}
      </Button>
    );
  return (
    <Tooltip title={label}>
      <FrontstageNodeActionButton
        {...props}
        ref={ref}
        aria-label={label}
        icon={icon ?? <MenuOutlined />}
        className={`block-design-button ${className ?? ''}`}
        style={style}
      />
    </Tooltip>
  );
});

/** Same compact menu density and surface as the canvas block's more-actions menu. */
export function BlockDesignMenu({ menu, styles, ...props }: DropdownProps) {
  return (
    <Dropdown
      trigger={['click']}
      {...props}
      styles={
        styles ?? {
          root: { minWidth: 208 },
          item: {
            height: 28,
            minHeight: 28,
            boxSizing: 'border-box',
            padding: '4px 8px',
            fontSize: 11,
            fontWeight: 400,
            lineHeight: '20px'
          }
        }
      }
      menu={{
        ...menu,
        style: {
          padding: 8,
          fontSize: 11,
          border: '1px solid var(--border-default, #d5ddd8)',
          borderRadius: 6,
          boxShadow: '0 6px 16px rgba(15, 23, 42, 0.12)',
          ...menu?.style
        }
      }}
    />
  );
}

export const BlockDesignToolbar = forwardRef<
  HTMLSpanElement,
  HTMLAttributes<HTMLSpanElement> & {
    tools: ReactNode;
    disabled?: boolean;
  }
>(function BlockDesignToolbar(
  { children, tools, disabled = false, className = '', style, ...props },
  ref
) {
  return (
    <span
      {...props}
      ref={ref}
      className={`block-design-surface ${disabled ? '' : 'block-design-surface-enabled'} ${className}`}
      style={
        {
          '--block-design-color': FRONTSTAGE_DESIGN_BLUE.borderHover,
          '--block-design-hover-bg': FRONTSTAGE_DESIGN_BLUE.bgHover,
          ...style
        } as CSSProperties
      }
    >
      {children}
      {!disabled && (
        <span className="block-design-toolbar" role="toolbar">
          {tools}
        </span>
      )}
    </span>
  );
});
