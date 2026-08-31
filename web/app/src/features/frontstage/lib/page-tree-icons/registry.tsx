import { lazy, Suspense } from 'react';
import type {
  ComponentType,
  CSSProperties,
  LazyExoticComponent,
  ReactNode
} from 'react';

import {
  hasPageTreeIconComponent,
  loadPageTreeIconComponent
} from 'virtual:1flowbase-page-tree-icon-runtime';

import { IconComponentCache } from './cache';

type PageTreeIconProps = {
  name?: string | null;
  className?: string;
  style?: CSSProperties;
  fallback?: ReactNode;
};

type LazyPageTreeIcon = LazyExoticComponent<
  ComponentType<{
    className?: string;
    style?: CSSProperties;
    'aria-hidden'?: boolean | 'true' | 'false';
  }>
>;

const MAX_CACHED_ICONS = 128;
const componentCache = new IconComponentCache<LazyPageTreeIcon>(
  MAX_CACHED_ICONS
);

function loadIcon(name: string) {
  if (!hasPageTreeIconComponent(name)) return null;
  return componentCache.getOrCreate(name, () =>
    lazy(async () => {
      const component = await loadPageTreeIconComponent(name);
      if (!component) {
        throw new Error(`Unknown page tree icon '${name}'`);
      }
      return { default: component };
    })
  );
}

function PageTreeIcon({
  name,
  className,
  style,
  fallback = null
}: PageTreeIconProps) {
  if (!name) {
    return <>{fallback}</>;
  }

  const Icon = loadIcon(name);
  if (!Icon) {
    return <>{fallback}</>;
  }

  return (
    <Suspense fallback={fallback}>
      <Icon aria-hidden className={className} style={style} />
    </Suspense>
  );
}

export { PageTreeIcon };
export type { PageTreeIconProps };
