import type { ElementType, HTMLAttributes, ReactNode } from 'react';

import './styles.css';

export interface SurfaceProps extends HTMLAttributes<HTMLElement> {
  readonly as?: ElementType;
  readonly children?: ReactNode;
}

export function Surface({
  as: Element = 'section',
  children,
  className,
  ...surfaceProps
}: SurfaceProps) {
  const surfaceClassName = ['oneflow-surface', className]
    .filter(Boolean)
    .join(' ');
  return (
    <Element className={surfaceClassName} {...surfaceProps}>
      {children}
    </Element>
  );
}

export interface ScrollableSurfaceProps extends SurfaceProps {}

export function ScrollableSurface({
  children,
  className,
  ...surfaceProps
}: ScrollableSurfaceProps) {
  const surfaceClassName = ['oneflow-scrollable-surface', className]
    .filter(Boolean)
    .join(' ');
  return (
    <Surface className={surfaceClassName} {...surfaceProps}>
      {children}
    </Surface>
  );
}
