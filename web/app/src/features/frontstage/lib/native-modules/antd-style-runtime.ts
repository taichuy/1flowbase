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
let artifactStyleInstanceSequence = 0;

export function loadAntdStyleModule(): Promise<typeof import('antd-style')> {
  moduleFlight ??= import('antd-style').catch((error: unknown) => {
    moduleFlight = undefined;
    throw error;
  });
  return moduleFlight;
}

export async function loadAntdStyleModuleForArtifact(): Promise<{
  module: Record<string, unknown>;
  readonly styles?: readonly { css: string }[];
}> {
  const antdStyleModule = await loadAntdStyleModule();
  if (typeof document === 'undefined') return { module: antdStyleModule };

  // Module-top-level styles belong to the evaluated artifact, not the host page.
  const styleContainer = document.createDocumentFragment();
  const artifactInstance = antdStyleModule.createInstance({
    container: styleContainer,
    key: nextArtifactStyleInstanceKey()
  });
  return {
    module: {
      ...antdStyleModule,
      createStaticStyles: artifactInstance.createStaticStyles
    },
    get styles() {
      const css = [...styleContainer.querySelectorAll('style')]
        .map((style) => style.textContent ?? '')
        .join('\n');
      return css === '' ? [] : [{ css }];
    }
  };
}

function nextArtifactStyleInstanceKey(): string {
  let sequence = ++artifactStyleInstanceSequence;
  let suffix = '';
  while (sequence > 0) {
    sequence -= 1;
    suffix = String.fromCharCode(97 + (sequence % 26)) + suffix;
    sequence = Math.floor(sequence / 26);
  }
  return `native-static-${suffix}`;
}
