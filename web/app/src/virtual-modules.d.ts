declare module 'virtual:1flowbase-page-tree-icon-previews' {
  export const pageTreeIconNames: readonly string[];
  export const pageTreeIconPackManifest: readonly {
    id: string;
    iconCount: number;
    sourceBytes: number;
  }[];
  export function pageTreeIconPreviewHref(name: string): string | null;
}

declare module 'virtual:1flowbase-page-tree-icon-runtime' {
  import type { ComponentType } from 'react';

  type PageTreeIconComponent = ComponentType<{
    className?: string;
    style?: import('react').CSSProperties;
    'aria-hidden'?: boolean | 'true' | 'false';
  }>;

  export function loadPageTreeIconComponent(
    name: string
  ): Promise<PageTreeIconComponent | null>;
  export function hasPageTreeIconComponent(name: string): boolean;
}

declare module 'virtual:1flowbase-dev-hmr-probe' {}
