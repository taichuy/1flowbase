import type { StyleBoundaryManifestScene } from './types';

declare global {
  interface Window {
    __STYLE_BOUNDARY__?: {
      ready: boolean;
      scene: StyleBoundaryManifestScene;
    };
    __STYLE_BOUNDARY_I18N_CATALOG_REQUESTS__?: Array<Record<string, string>>;
  }
}

export {};
