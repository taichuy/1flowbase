declare module 'virtual:1flowbase-page-tree-icons' {
  import type { ComponentType } from 'react';

  type PageTreeIconComponent = ComponentType<{
    className?: string;
    style?: import('react').CSSProperties;
    'aria-hidden'?: boolean | 'true' | 'false';
  }>;

  export const pageTreeIconNames: readonly string[];
  export const pageTreeIconLoaders: Readonly<
    Record<string, () => Promise<PageTreeIconComponent>>
  >;
}

declare module 'virtual:1flowbase-dev-hmr-probe' {}
