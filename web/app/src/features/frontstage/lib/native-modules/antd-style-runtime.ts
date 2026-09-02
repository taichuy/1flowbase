import type { ComponentType, ReactNode } from 'react';

export const ANTD_STYLE_EXPORTS = [
  'StyleProvider',
  'ThemeProvider',
  'createGlobalStyle',
  'createInstance',
  'createStaticStyles',
  'createStaticStylesFactory',
  'createStyles',
  'createStylish',
  'css',
  'cssVar',
  'cx',
  'extractStaticStyle',
  'injectGlobal',
  'keyframes',
  'legacyLogicalPropertiesTransformer',
  'px2remTransformer',
  'responsive',
  'setupStyled',
  'styleManager',
  'useAntdStylish',
  'useAntdTheme',
  'useAntdToken',
  'useResponsive',
  'useTheme',
  'useThemeMode'
] as const;

export interface AntdStyleShadowProviderProps {
  children: ReactNode;
  container: ShadowRoot;
  prefix: string;
}

export type AntdStyleShadowProvider =
  ComponentType<AntdStyleShadowProviderProps>;

let moduleFlight: Promise<typeof import('antd-style')> | undefined;

export function loadAntdStyleModule(): Promise<typeof import('antd-style')> {
  moduleFlight ??= import('antd-style').catch((error: unknown) => {
    moduleFlight = undefined;
    throw error;
  });
  return moduleFlight;
}
