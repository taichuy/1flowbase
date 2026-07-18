import { Button } from 'antd';
import type { CSSProperties, ReactNode } from 'react';

import type { DockPanelSchema } from '../contracts/overlay-shell-schema';
import { i18nText } from '../../../i18n/text';

export function SchemaDockPanel({
  schema,
  width,
  children,
  footer,
  onClose,
  className,
  bodyClassName,
  headerExtra,
  style,
  headerless = false
}: {
  schema: DockPanelSchema;
  width?: number;
  children: ReactNode;
  footer?: ReactNode;
  onClose?: () => void;
  className?: string;
  bodyClassName?: string;
  headerExtra?: ReactNode;
  style?: CSSProperties;
  headerless?: boolean;
}) {
  const resolvedWidth = width ?? schema.width;

  return (
    <aside
      aria-label={schema.title}
      className={className}
      style={{
        ...style,
        ...(resolvedWidth != null ? { width: `${resolvedWidth}px` } : {})
      }}
    >
      {headerless ? null : (
        <header>
          <div>{schema.title}</div>
          {headerExtra}
          {onClose ? (
            <Button aria-label={i18nText("schemaUi", "auto.close_named_panel", { value1: schema.title })} type="text" onClick={onClose}>
              {i18nText("schemaUi", "auto.close")}</Button>
          ) : null}
        </header>
      )}
      <div className={bodyClassName}>{children}</div>
      {footer ? <footer>{footer}</footer> : null}
    </aside>
  );
}
