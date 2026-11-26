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

declare module 'virtual:1flowbase-default-login-artifact' {
  const artifact:
    | import('@1flowbase/page-runtime/browser').NativeReactComponentArtifact
    | null;
  export default artifact;
}

declare module 'virtual:1flowbase-auth-translations' {
  type Auth = Pick<
    typeof import('./features/auth/i18n/en_US.json'),
    'sign_in' | 'external_auth' | 'auto'
  >;
  export const en_US: Auth;
  export const zh_Hans: Auth;
}

declare module 'virtual:1flowbase-opening-copy' {
  export const enTagline: string;
  export const zhTagline: string;
  export const enFooter: string;
  export const zhFooter: string;
}
