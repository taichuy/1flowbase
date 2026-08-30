import { lazy, Suspense } from 'react';
import type {
  ComponentType,
  CSSProperties,
  LazyExoticComponent,
  ReactNode
} from 'react';

import {
  pageTreeIconLoaders,
  pageTreeIconNames
} from 'virtual:1flowbase-page-tree-icons';

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
const componentCache = new Map<string, LazyPageTreeIcon>();

function loadIcon(name: string) {
  const loader = pageTreeIconLoaders[name];
  if (!loader) {
    return null;
  }

  const cached = componentCache.get(name);
  if (cached) {
    componentCache.delete(name);
    componentCache.set(name, cached);
    return cached;
  }

  const component = lazy(async () => ({ default: await loader() }));
  componentCache.set(name, component);
  while (componentCache.size > MAX_CACHED_ICONS) {
    const oldest = componentCache.keys().next().value;
    if (!oldest) break;
    componentCache.delete(oldest);
  }
  return component;
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

export { PageTreeIcon, pageTreeIconNames };
export type { PageTreeIconProps };
