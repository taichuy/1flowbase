import { lazy, Suspense } from 'react';
import type {
  ComponentType,
  CSSProperties,
  LazyExoticComponent,
  ReactNode
} from 'react';

import { pageTreeIconLoaders } from 'virtual:1flowbase-page-tree-icons';

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
  const loader = pageTreeIconLoaders[name];
  if (!loader) {
    return null;
  }

  return componentCache.getOrCreate(name, () =>
    lazy(async () => ({ default: await loader() }))
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
